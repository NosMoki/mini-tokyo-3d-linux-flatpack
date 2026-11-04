/* eslint-disable import-x/no-unresolved -- gi:// and GJS built-in modules are resolved by GJS */
import Adw from 'gi://Adw?version=1';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Gtk from 'gi://Gtk?version=4.0';
import Gettext from 'gettext';

import {MiniTokyo3DPreferencesDialog} from './preferences.js';
import {MiniTokyo3DWindow} from './window.js';

const _ = Gettext.gettext;

// Command-line options for the URL query parameters that the web page reads
// (see gnome/web/index.html.in).
const QUERY_OPTIONS = [{
    name: 'lang',
    param: 'lang',
    description: _('Show the map in the given language, such as “en” or “ja”'),
    argDescription: _('LANGUAGE')
}, {
    name: 'selection',
    param: 'selection',
    description: _('Select the given train, flight or station on startup'),
    argDescription: _('ID')
}, {
    name: 'gtfs-url',
    param: 'gtfsurl',
    description: _('Show the given GTFS feed instead of the Tokyo data (requires --gtfs-color)'),
    argDescription: _('URL')
}, {
    name: 'gtfs-vehicle-position-url',
    param: 'gtfsvpurl',
    description: _('Use the given GTFS Realtime vehicle positions for the GTFS feed'),
    argDescription: _('URL')
}, {
    name: 'gtfs-color',
    param: 'gtfscolor',
    description: _('Draw the GTFS feed in the given color'),
    argDescription: _('RRGGBB')
}];

export const MiniTokyo3DApplication = GObject.registerClass(
class MiniTokyo3DApplication extends Adw.Application {

    constructor(config) {
        super({
            applicationId: config.appId,
            version: config.version,
            flags: Gio.ApplicationFlags.HANDLES_COMMAND_LINE,
            resourceBasePath: '/com/minitokyo3d/MiniTokyo3D'
        });

        this.config = config;

        GLib.set_application_name(_('Mini Tokyo 3D'));

        this.set_option_context_parameter_string(_('[URL]'));
        this.set_option_context_summary(_('Opens a minitokyo3d.com link, such as a shared train, flight or station, when one is given.'));
        for (const option of QUERY_OPTIONS) {
            this.add_main_option(option.name, 0, GLib.OptionFlags.NONE, GLib.OptionArg.STRING,
                option.description, option.argDescription);
        }

        const actions = [
            ['quit', () => this.quit(), ['<Primary>q']],
            ['preferences', () => this._showPreferences(), ['<Primary>comma']],
            ['about', () => this._showAbout(), []]
        ];

        for (const [name, callback, accels] of actions) {
            const action = new Gio.SimpleAction({name});

            action.connect('activate', callback);
            this.add_action(action);
            this.set_accels_for_action(`app.${name}`, accels);
        }
        this.set_accels_for_action('win.reload', ['<Primary>r', 'F5']);
        this.set_accels_for_action('win.fullscreen', ['F11']);
        this.set_accels_for_action('window.close', ['<Primary>w']);
    }

    _getWindow() {
        let window = this.active_window;

        if (!window) {
            window = new MiniTokyo3DWindow(this);
            window.loadMap();
        }
        return window;
    }

    vfunc_activate() { // eslint-disable-line camelcase
        this._getWindow().present();
    }

    vfunc_command_line(commandLine) { // eslint-disable-line camelcase
        const options = commandLine.get_options_dict(),
            args = commandLine.get_arguments().slice(1),
            params = [];
        let query = null,
            fragment = null;

        // A minitokyo3d.com link carries the same query and fragment as the page.
        if (args.length > 0) {
            try {
                const uri = GLib.Uri.parse(args[0], GLib.UriFlags.NONE);

                query = uri.get_query();
                fragment = uri.get_fragment();
            } catch (error) {
                commandLine.printerr(`${_('Not a valid URL:')} ${args[0]}\n`);
                return 1;
            }
        }

        // Options come first, so they take precedence over the link's parameters.
        for (const option of QUERY_OPTIONS) {
            const value = options.lookup_value(option.name, new GLib.VariantType('s'));

            if (value) {
                params.push(`${option.param}=${encodeURIComponent(value.unpack())}`);
            }
        }
        if (query) {
            params.push(query);
        }

        const location = (params.length > 0 ? `?${params.join('&')}` : '') + (fragment ? `#${fragment}` : ''),
            existing = this.active_window;

        if (existing) {
            if (location) {
                existing.loadMap(location);
            }
            existing.present();
        } else {
            const window = new MiniTokyo3DWindow(this);

            window.loadMap(location);
            window.present();
        }
        return 0;
    }

    _showPreferences() {
        const window = this._getWindow(),
            dialog = new MiniTokyo3DPreferencesDialog(new Gio.Settings({schemaId: this.config.appId}));

        dialog.connect('settings-changed', () => window.loadMap());
        dialog.present(window);
    }

    _showAbout() {
        const about = new Adw.AboutDialog({
            applicationName: _('Mini Tokyo 3D'),
            applicationIcon: this.config.appId,
            developerName: 'Akihiko Kusanagi',
            version: this.config.version,
            comments: _('A real-time 3D digital map of Tokyo’s public transport system'),
            website: 'https://minitokyo3d.com',
            issueUrl: 'https://github.com/nagix/mini-tokyo-3d/issues',
            developers: ['Akihiko Kusanagi https://github.com/nagix'],
            copyright: '© 2019-2026 Akihiko Kusanagi',
            licenseType: Gtk.License.MIT_X11,
            // Translators: Replace "translator-credits" with your name/username, and optionally an email or URL.
            translatorCredits: _('translator-credits')
        });

        about.add_link(_('_User Guide'), 'https://minitokyo3d.com/docs/master/');
        about.add_link(_('_Developer Guide'), 'https://minitokyo3d.com/docs/master/developer-guide/');
        about.add_link(_('_Donate'), 'https://github.com/sponsors/nagix');
        about.add_credit_section(_('Data'), ['Public Transportation Open Data Center https://www.odpt.org']);
        about.add_legal_section('Mapbox GL JS', '© Mapbox', Gtk.License.CUSTOM,
            'Mapbox GL JS is subject to the <a href="https://www.mapbox.com/legal/tos">Mapbox Terms of Service</a>.');
        about.add_legal_section('deck.gl', '© vis.gl contributors', Gtk.License.MIT_X11, null);
        about.add_legal_section('three.js', '© three.js authors', Gtk.License.MIT_X11, null);
        about.present(this._getWindow());
    }
});

export function main(argv, config) {
    return new MiniTokyo3DApplication(config).runAsync(argv);
}
