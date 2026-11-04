/* eslint-disable import-x/no-unresolved -- gi:// and GJS built-in modules are resolved by GJS */
import Adw from 'gi://Adw?version=1';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Gtk from 'gi://Gtk?version=4.0';
import WebKit from 'gi://WebKit?version=6.0';
import Gettext from 'gettext';

import {APP_URI, isAppUri, registerAppScheme} from './scheme.js';
import {lookupTokens} from './secrets.js';

const _ = Gettext.gettext;

Gio._promisify(WebKit.WebView.prototype, 'evaluate_javascript', 'evaluate_javascript_finish');
Gio._promisify(WebKit.WebView.prototype, 'call_async_javascript_function', 'call_async_javascript_function_finish');
Gio._promisify(Gtk.UriLauncher.prototype, 'launch', 'launch_finish');

// Context menu entries that make sense inside the app. Browser entries such as
// Back, Reload or Open Link in New Window are dropped.
const CONTEXT_MENU_ACTIONS = new Set([
    WebKit.ContextMenuAction.COPY,
    WebKit.ContextMenuAction.CUT,
    WebKit.ContextMenuAction.PASTE,
    WebKit.ContextMenuAction.PASTE_AS_PLAIN_TEXT,
    WebKit.ContextMenuAction.DELETE,
    WebKit.ContextMenuAction.SELECT_ALL,
    WebKit.ContextMenuAction.INSERT_EMOJI,
    WebKit.ContextMenuAction.INPUT_METHODS,
    WebKit.ContextMenuAction.UNICODE,
    WebKit.ContextMenuAction.SPELLING_GUESS,
    WebKit.ContextMenuAction.NO_GUESSES_FOUND,
    WebKit.ContextMenuAction.IGNORE_SPELLING,
    WebKit.ContextMenuAction.LEARN_SPELLING,
    WebKit.ContextMenuAction.IGNORE_GRAMMAR,
    WebKit.ContextMenuAction.COPY_LINK_TO_CLIPBOARD,
    WebKit.ContextMenuAction.COPY_IMAGE_TO_CLIPBOARD,
    WebKit.ContextMenuAction.COPY_IMAGE_URL_TO_CLIPBOARD,
    WebKit.ContextMenuAction.INSPECT_ELEMENT
]);

// URI schemes that links may open in the user's default apps
const EXTERNAL_SCHEMES = ['http', 'https', 'mailto'];

// Toggles fullscreen through the map's own fullscreen button, so the button and
// the page stay in step with the window. Returns false when the map isn't
// ready yet. Run with call_async_javascript_function(), which (unlike
// evaluate_javascript()) counts as a user gesture, as requestFullscreen() needs.
const TOGGLE_FULLSCREEN_FUNCTION = `
    if (document.fullscreenElement) {
        await document.exitFullscreen();
        return true;
    }
    const button = document.querySelector('.mapboxgl-ctrl-fullscreen');
    if (button) {
        button.click();
        return true;
    }
    return false;`;

const EXIT_FULLSCREEN_SCRIPT = 'if (document.fullscreenElement) { document.exitFullscreen().catch(() => {}); }';

let networkSession = null;

// One persistent network session per process keeps the HTTP cache, including
// map tiles, across launches.
function getNetworkSession(appId) {
    if (!networkSession) {
        networkSession = WebKit.NetworkSession.new(
            GLib.build_filenamev([GLib.get_user_data_dir(), appId, 'webkit']),
            GLib.build_filenamev([GLib.get_user_cache_dir(), appId, 'webkit']));
    }
    return networkSession;
}

function filterContextMenu(menu) {
    let previousIsSeparator = true;

    for (const item of menu.get_items()) {
        if (item.is_separator()) {
            if (previousIsSeparator) {
                menu.remove(item);
            }
            previousIsSeparator = true;
        } else if (CONTEXT_MENU_ACTIONS.has(item.get_stock_action())) {
            previousIsSeparator = false;
        } else {
            menu.remove(item);
        }
    }

    const last = menu.last();

    if (last && last.is_separator()) {
        menu.remove(last);
    }
}

export const MiniTokyo3DWindow = GObject.registerClass({
    GTypeName: 'MiniTokyo3DWindow',
    Template: 'resource:///com/minitokyo3d/MiniTokyo3D/window.ui',
    InternalChildren: ['toastOverlay', 'toolbarView', 'stack']
}, class MiniTokyo3DWindow extends Adw.ApplicationWindow {

    constructor(application) {
        super({application});

        const config = application.config;

        this._config = config;
        this._location = '';
        this._loadSerial = 0;

        this._settings = new Gio.Settings({schemaId: config.appId});
        this._settings.bind('window-width', this, 'default-width', Gio.SettingsBindFlags.DEFAULT);
        this._settings.bind('window-height', this, 'default-height', Gio.SettingsBindFlags.DEFAULT);
        this._settings.bind('window-maximized', this, 'maximized', Gio.SettingsBindFlags.DEFAULT);

        this._webView = this._createWebView();
        this._stack.add_named(this._webView, 'map');
        this._stack.visibleChildName = 'loading';

        const reloadAction = new Gio.SimpleAction({name: 'reload'}),
            fullscreenAction = new Gio.SimpleAction({name: 'fullscreen'});

        reloadAction.connect('activate', () => this.reload());
        fullscreenAction.connect('activate', () => this.toggleFullscreen());
        this.add_action(reloadAction);
        this.add_action(fullscreenAction);

        // If the window leaves fullscreen by other means, bring the header bar
        // back and let the page leave fullscreen too.
        this.connect('notify::fullscreened', () => {
            if (!this.fullscreened) {
                this._toolbarView.revealTopBars = true;
                this._webView.evaluate_javascript(EXIT_FULLSCREEN_SCRIPT, -1, null, null, null).catch(() => {});
            }
        });
    }

    _createWebView() {
        const config = this._config,
            devtools = Boolean(GLib.getenv('MT3D_DEVTOOLS')),
            context = WebKit.WebContext.get_default();

        registerAppScheme(context, config.webDir);

        const webView = new WebKit.WebView({
            webContext: context,
            networkSession: getNetworkSession(config.appId),
            userContentManager: new WebKit.UserContentManager(),
            settings: new WebKit.Settings({
                enableDeveloperExtras: devtools,
                enableWriteConsoleMessagesToStdout: devtools,
                javascriptCanOpenWindowsAutomatically: false,
                enableBackForwardNavigationGestures: false
            }),
            hexpand: true,
            vexpand: true
        });

        webView.connect('decide-policy', this._onDecidePolicy.bind(this));

        webView.connect('context-menu', (view, menu) => {
            filterContextMenu(menu);
            return menu.get_n_items() === 0;
        });

        // The map's fullscreen button fullscreens the whole window; hide the
        // header bar while it is.
        webView.connect('enter-fullscreen', () => {
            this._toolbarView.revealTopBars = false;
            return false;
        });
        webView.connect('leave-fullscreen', () => {
            this._toolbarView.revealTopBars = true;
            return false;
        });

        webView.connect('web-process-terminated', (view, reason) => {
            if (reason !== WebKit.WebProcessTerminationReason.TERMINATED_BY_API) {
                this._toastOverlay.add_toast(new Adw.Toast({
                    title: _('The map stopped unexpectedly'),
                    buttonLabel: _('_Reload'),
                    actionName: 'win.reload',
                    timeout: 0
                }));
            }
        });

        return webView;
    }

    _onDecidePolicy(webView, decision, type) {
        const {NAVIGATION_ACTION, NEW_WINDOW_ACTION, RESPONSE} = WebKit.PolicyDecisionType;

        // The page itself never navigates away; refuse anything that would
        // replace it, such as a link or file dropped onto the window.
        if (type === RESPONSE) {
            const uri = decision.get_request().get_uri();

            if (decision.is_main_frame_main_resource() && !isAppUri(uri) && uri !== 'about:blank') {
                decision.ignore();
                return true;
            }
            return false;
        }

        if (type !== NAVIGATION_ACTION && type !== NEW_WINDOW_ACTION) {
            return false;
        }

        const action = decision.get_navigation_action(),
            uri = action.get_request().get_uri();

        // Let the page load its own content and frames (e.g. a plugin's embedded
        // video), but send clicked links and new windows to the default browser.
        if (type === NAVIGATION_ACTION && (isAppUri(uri) || action.get_navigation_type() !== WebKit.NavigationType.LINK_CLICKED)) {
            return false;
        }

        decision.ignore();
        if (action.is_user_gesture()) {
            this._openExternal(uri);
        }
        return true;
    }

    async _openExternal(uri) {
        if (!EXTERNAL_SCHEMES.includes(GLib.Uri.peek_scheme(uri))) {
            return;
        }
        try {
            await new Gtk.UriLauncher({uri}).launch(this, null);
        } catch (error) {
            if (!error.matches(Gtk.DialogError, Gtk.DialogError.DISMISSED)) {
                console.error(error);
                this.addToast(_('Could not open the link'));
            }
        }
    }

    addToast(title) {
        this._toastOverlay.add_toast(new Adw.Toast({title}));
    }

    // Loads the map, with the tokens from the keyring and the language from
    // Preferences. location is the URL query and fragment passed on to the page
    // (see the command-line options in main.js); it is kept for later reloads.
    async loadMap(location) {
        if (typeof location === 'string') {
            this._location = location;
        }

        const serial = ++this._loadSerial;
        let tokens;

        try {
            tokens = await lookupTokens();
        } catch (error) {
            console.error(error);
            this.addToast(_('Could not read the access tokens from the keyring'));
            tokens = {};
        }

        // A newer call has taken over while the keyring was being read.
        if (serial !== this._loadSerial) {
            return;
        }

        if (!tokens.mapbox) {
            this._webView.load_uri('about:blank');
            this._stack.visibleChildName = 'token';
            return;
        }

        const desktop = {accessToken: tokens.mapbox},
            secrets = {},
            lang = this._settings.get_string('language');

        if (tokens.odpt) {
            secrets.odpt = tokens.odpt;
        }
        if (tokens.challenge) {
            secrets.challenge = tokens.challenge;
        }
        if (Object.keys(secrets).length > 0) {
            desktop.secrets = secrets;
        }
        if (lang) {
            desktop.lang = lang;
        }

        const contentManager = this._webView.userContentManager;

        contentManager.remove_all_scripts();
        contentManager.add_script(WebKit.UserScript.new(
            `window.mt3dDesktop = ${JSON.stringify(desktop)};`,
            WebKit.UserContentInjectedFrames.TOP_FRAME,
            WebKit.UserScriptInjectionTime.START,
            [`${APP_URI}*`],
            null));

        this._webView.load_uri(`${APP_URI}index.html${this._location}`);
        this._stack.visibleChildName = 'map';
        this._webView.grab_focus();
    }

    reload() {
        if (this._stack.visibleChild === this._webView) {
            this._webView.reload();
        } else {
            this.loadMap();
        }
    }

    async toggleFullscreen() {
        if (this._stack.visibleChild === this._webView) {
            try {
                const result = await this._webView.call_async_javascript_function(TOGGLE_FULLSCREEN_FUNCTION, -1, null, null, null, null);

                if (result.to_boolean()) {
                    return;
                }
            } catch (error) {
                console.error(error);
            }
        }
        if (this.fullscreened) {
            this.unfullscreen();
        } else {
            this.fullscreen();
        }
    }
});
