//! The main window: a header bar over the Mini Tokyo 3D web app, plus the
//! pages shown before the map can load (token setup) or when it can't.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};
use webkit::prelude::*;

use crate::secrets::{self, Secret, Verdict};
use crate::server::{Server, Tokens};
use crate::{APP_NAME, LaunchOptions, dev, preferences, shortcuts};

pub struct Window {
    pub window: adw::ApplicationWindow,
    pub settings: gio::Settings,
    toast_overlay: adw::ToastOverlay,
    toolbar_view: adw::ToolbarView,
    stack: gtk::Stack,
    pub web_view: webkit::WebView,
    token_entry: adw::PasswordEntryRow,
    token_error: gtk::Label,
    token_button: gtk::Button,
    error_page: adw::StatusPage,
    error_button: gtk::Button,
    server: Option<Server>,
    launch: RefCell<LaunchOptions>,
    /// Whether tokens can be saved, i.e. whether a keyring answered.
    keyring_available: std::cell::Cell<bool>,
}

impl Window {
    pub fn new(app: &adw::Application, settings: gio::Settings, launch: LaunchOptions) -> Rc<Window> {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title(APP_NAME)
            .icon_name(crate::APP_ID)
            .default_width(settings.int("window-width"))
            .default_height(settings.int("window-height"))
            .maximized(settings.boolean("window-maximized"))
            .width_request(360)
            .height_request(294)
            .build();

        let menu = main_menu();
        let menu_button = gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .menu_model(&menu)
            .primary(true)
            .tooltip_text("Main Menu")
            .build();
        let header_bar = adw::HeaderBar::new();
        header_bar.pack_end(&menu_button);

        let web_view = create_web_view();

        let (token_page, token_entry, token_error, token_button) = token_page();

        let error_button = gtk::Button::builder()
            .label("Reload")
            .halign(gtk::Align::Center)
            .css_classes(["pill", "suggested-action"])
            .build();
        let error_page = adw::StatusPage::builder()
            .icon_name("dialog-error-symbolic")
            .child(&error_button)
            .build();

        let spinner = adw::Spinner::builder()
            .width_request(32)
            .height_request(32)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .build();

        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .build();
        stack.add_named(&spinner, Some("loading"));
        stack.add_named(&token_page, Some("token"));
        stack.add_named(&web_view, Some("map"));
        stack.add_named(&error_page, Some("error"));

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header_bar);
        toolbar_view.set_content(Some(&stack));

        let toast_overlay = adw::ToastOverlay::new();
        toast_overlay.set_child(Some(&toolbar_view));
        window.set_content(Some(&toast_overlay));

        let server = match web_dir() {
            Some(dir) => match Server::start(&dir) {
                Ok(server) => Some(server),
                Err(error) => {
                    eprintln!("Couldn’t start the local web server: {error}");
                    None
                }
            },
            None => None,
        };

        let this = Rc::new(Window {
            window,
            settings,
            toast_overlay,
            toolbar_view,
            stack,
            web_view,
            token_entry,
            token_error,
            token_button,
            error_page,
            error_button,
            server,
            launch: RefCell::new(launch),
            keyring_available: std::cell::Cell::new(true),
        });
        this.connect_signals();
        this.add_actions();
        dev::attach(&this);
        this.start();
        this
    }

    fn connect_signals(self: &Rc<Self>) {
        self.window.connect_close_request(glib::clone!(
            #[weak(rename_to = this)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |window| {
                if !window.is_fullscreen() {
                    let (width, height) = window.default_size();
                    let _ = this.settings.set_int("window-width", width);
                    let _ = this.settings.set_int("window-height", height);
                    let _ = this.settings.set_boolean("window-maximized", window.is_maximized());
                }
                glib::Propagation::Proceed
            }
        ));

        // Hide the header bar in fullscreen, whether it was asked for with F11
        // or by the map's own fullscreen button.
        self.window.connect_fullscreened_notify(glib::clone!(
            #[weak(rename_to = this)]
            self,
            move |window| this.toolbar_view.set_reveal_top_bars(!window.is_fullscreen())
        ));

        self.token_entry.connect_changed(glib::clone!(
            #[weak(rename_to = this)]
            self,
            move |entry| {
                this.token_button.set_sensitive(!entry.text().trim().is_empty());
                this.token_error.set_visible(false);
                entry.remove_css_class("error");
            }
        ));
        self.token_entry.connect_entry_activated(glib::clone!(
            #[weak(rename_to = this)]
            self,
            move |_| this.submit_token()
        ));
        self.token_button.connect_clicked(glib::clone!(
            #[weak(rename_to = this)]
            self,
            move |_| this.submit_token()
        ));
        self.error_button.connect_clicked(glib::clone!(
            #[weak(rename_to = this)]
            self,
            move |_| this.start()
        ));

        let web_view = &self.web_view;
        web_view.connect_decide_policy(glib::clone!(
            #[weak(rename_to = this)]
            self,
            #[upgrade_or]
            false,
            move |_, decision, kind| this.decide_policy(decision, kind)
        ));
        // window.open() and target="_blank" links open in the web browser.
        web_view.connect_create(glib::clone!(
            #[weak(rename_to = this)]
            self,
            #[upgrade_or]
            None,
            move |_, action| {
                if let Some(uri) = action.request().and_then(|request| request.uri()) {
                    this.open_external(&uri);
                }
                None
            }
        ));
        // The map uses the right button to rotate, so only offer a context
        // menu where it helps: in text fields and on selected text.
        web_view.connect_context_menu(|_, _, hit| {
            let useful = hit.context_is_editable() || hit.context_is_selection() || cfg!(debug_assertions);
            !useful
        });
        web_view.connect_permission_request(|_, request| {
            request.deny();
            true
        });
        web_view.connect_web_process_terminated(glib::clone!(
            #[weak(rename_to = this)]
            self,
            move |_, reason| {
                let description = match reason {
                    webkit::WebProcessTerminationReason::ExceededMemoryLimit => {
                        "The map ran out of memory. Turning off heavy layers such as PLATEAU buildings can help."
                    }
                    _ => "The part of Mini Tokyo 3D that draws the map has stopped.",
                };
                this.show_error("The Map Stopped Unexpectedly", description, true);
            }
        ));
        web_view.connect_load_failed(glib::clone!(
            #[weak(rename_to = this)]
            self,
            #[upgrade_or]
            false,
            move |_, _, uri, error| {
                let ours = this
                    .server
                    .as_ref()
                    .is_some_and(|server| uri.starts_with(&server.base_uri()));
                let interrupted = error.matches(webkit::NetworkError::Cancelled)
                    || error.matches(webkit::PolicyError::FrameLoadInterruptedByPolicyChange);
                if ours && !interrupted {
                    this.show_error("Couldn’t Load the Map", error.message(), true);
                }
                false
            }
        ));
    }

    fn add_actions(self: &Rc<Self>) {
        let fullscreen = gio::ActionEntry::builder("fullscreen")
            .activate(|window: &adw::ApplicationWindow, _, _| {
                if window.is_fullscreen() {
                    window.unfullscreen();
                } else {
                    window.fullscreen();
                }
            })
            .build();
        let weak = Rc::downgrade(self);
        let reload = gio::ActionEntry::builder("reload")
            .activate(move |_: &adw::ApplicationWindow, _, _| {
                if let Some(this) = weak.upgrade() {
                    this.reload();
                }
            })
            .build();
        let weak = Rc::downgrade(self);
        let preferences = gio::ActionEntry::builder("preferences")
            .activate(move |_: &adw::ApplicationWindow, _, _| {
                if let Some(this) = weak.upgrade() {
                    preferences::present(&this);
                }
            })
            .build();
        let shortcuts = gio::ActionEntry::builder("shortcuts")
            .activate(|window: &adw::ApplicationWindow, _, _| shortcuts::present(window))
            .build();
        let about = gio::ActionEntry::builder("about")
            .activate(|window: &adw::ApplicationWindow, _, _| crate::present_about(window))
            .build();
        self.window
            .add_action_entries([fullscreen, reload, preferences, shortcuts, about]);
    }

    /// Looks up the tokens, then shows the map or the token setup page.
    fn start(self: &Rc<Self>) {
        if self.server.is_none() {
            let description = match web_dir() {
                Some(_) => "The local web server couldn’t be started.".to_owned(),
                None => format!(
                    "The web app files are missing. Run “node gnome/build-web.mjs” in the source tree, or set MT3D_WEB_DIR. Looked in: {}",
                    web_dir_candidates()
                        .iter()
                        .map(|path| path.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            };
            self.show_error("Can’t Start Mini Tokyo 3D", &description, false);
            return;
        }

        self.stack.set_visible_child_name("loading");
        let this = Rc::clone(self);
        glib::spawn_future_local(async move {
            let tokens = match dev::token_override() {
                Some(tokens) => tokens,
                None => match secrets::load_all().await {
                    Ok(tokens) => tokens,
                    Err(error) => {
                        eprintln!("Couldn’t read the keyring: {error}");
                        this.keyring_available.set(false);
                        this.toast("Couldn’t open GNOME Keyring. Tokens won’t be remembered.");
                        Tokens::default()
                    }
                },
            };
            this.set_tokens(tokens);
            this.show_map_or_setup();
        });
    }

    fn show_map_or_setup(&self) {
        if self.tokens().mapbox.is_empty() {
            self.stack.set_visible_child_name("token");
            self.token_entry.grab_focus();
        } else {
            self.load_map();
        }
    }

    pub fn tokens(&self) -> Tokens {
        self.server.as_ref().map(Server::tokens).unwrap_or_default()
    }

    fn set_tokens(&self, tokens: Tokens) {
        if let Some(server) = &self.server {
            server.set_tokens(tokens);
        }
    }

    /// Saves one token in the keyring and starts using it.
    pub async fn save_token(&self, secret: Secret, value: &str) -> Result<(), glib::Error> {
        let result = if self.keyring_available.get() {
            secrets::store(secret, value).await
        } else {
            Ok(())
        };
        let mut tokens = self.tokens();
        secret.set(&mut tokens, value.to_owned());
        self.set_tokens(tokens);
        result
    }

    fn submit_token(self: &Rc<Self>) {
        let token = self.token_entry.text().trim().to_owned();
        if let Err(message) = secrets::check_mapbox_token(&token) {
            self.show_token_error(message);
            return;
        }
        self.token_button.set_sensitive(false);
        self.token_button.set_label("Checking…");
        let this = Rc::clone(self);
        glib::spawn_future_local(async move {
            let verdict = secrets::verify_mapbox_token(&token).await;
            this.token_button.set_label("Continue");
            this.token_button.set_sensitive(true);
            match verdict {
                Verdict::Invalid(code) => {
                    this.show_token_error(secrets::describe_mapbox_code(&code));
                    return;
                }
                Verdict::Unchecked => this.toast("Couldn’t reach Mapbox to check the token"),
                Verdict::Valid => {}
            }
            if let Err(error) = this.save_token(Secret::Mapbox, &token).await {
                eprintln!("Couldn’t save the token: {error}");
                this.toast("Couldn’t save the token in GNOME Keyring");
            }
            this.token_entry.set_text("");
            this.load_map();
        });
    }

    fn show_token_error(&self, message: &str) {
        self.token_error.set_label(message);
        self.token_error.set_visible(true);
        self.token_entry.add_css_class("error");
        self.token_entry.grab_focus();
    }

    /// The page's address, with the language and the command line options.
    fn map_uri(&self) -> Option<String> {
        let server = self.server.as_ref()?;
        let launch = self.launch.borrow();
        let language = self.settings.string("language");
        let lang = launch
            .lang
            .clone()
            .or_else(|| (!language.is_empty()).then(|| language.to_string()));

        let mut query = Vec::new();
        let mut add = |name: &str, value: &Option<String>| {
            if let Some(value) = value {
                query.push(format!("{name}={}", glib::Uri::escape_string(value, None, false)));
            }
        };
        add("lang", &lang);
        add("selection", &launch.selection);
        add("gtfsurl", &launch.gtfs_url);
        add("gtfsvpurl", &launch.gtfs_vp_url);
        add(
            "gtfscolor",
            &launch
                .gtfs_color
                .as_ref()
                .map(|color| color.trim_start_matches('#').to_owned()),
        );

        let mut uri = format!("{}index.html", server.base_uri());
        if !query.is_empty() {
            uri.push('?');
            uri.push_str(&query.join("&"));
        }
        Some(uri)
    }

    /// Loads (or reloads) the map with the current settings.
    pub fn load_map(&self) {
        if self.tokens().mapbox.is_empty() {
            self.show_map_or_setup();
            return;
        }
        if let Some(uri) = self.map_uri() {
            self.web_view.load_uri(&uri);
            self.stack.set_visible_child_name("map");
            self.web_view.grab_focus();
        }
    }

    pub fn reload(&self) {
        match self.stack.visible_child_name().as_deref() {
            Some("map") => self.web_view.reload(),
            Some("error") => self.error_button.emit_clicked(),
            _ => {}
        }
    }

    /// Applies options from a later launch, like `mini-tokyo-3d --lang ja`.
    pub fn apply_launch_options(&self, launch: LaunchOptions) {
        if launch.is_empty() {
            return;
        }
        *self.launch.borrow_mut() = launch;
        if self.stack.visible_child_name().as_deref() == Some("map") {
            self.load_map();
        }
    }

    fn show_error(&self, title: &str, description: &str, can_reload: bool) {
        self.error_page.set_title(title);
        self.error_page
            .set_description(Some(&glib::markup_escape_text(description)));
        self.error_button.set_visible(can_reload);
        self.stack.set_visible_child_name("error");
    }

    pub fn toast(&self, title: &str) {
        self.toast_overlay.add_toast(adw::Toast::new(title));
    }

    fn is_app_uri(&self, uri: &str) -> bool {
        self.server
            .as_ref()
            .is_some_and(|server| uri.starts_with(&server.base_uri()))
    }

    fn decide_policy(&self, decision: &webkit::PolicyDecision, kind: webkit::PolicyDecisionType) -> bool {
        let new_window = match kind {
            webkit::PolicyDecisionType::NavigationAction => false,
            webkit::PolicyDecisionType::NewWindowAction => true,
            _ => return false,
        };
        let Some(action) = decision
            .downcast_ref::<webkit::NavigationPolicyDecision>()
            .and_then(|decision| decision.navigation_action())
        else {
            return false;
        };
        let Some(uri) = action.request().and_then(|request| request.uri()) else {
            return false;
        };
        if self.is_app_uri(&uri) {
            return false;
        }
        // Links that leave the map open in the web browser. Other loads, such as
        // the live camera players in their frames, stay in the page.
        if new_window || action.navigation_type() == webkit::NavigationType::LinkClicked {
            self.open_external(&uri);
            decision.ignore();
            return true;
        }
        false
    }

    fn open_external(&self, uri: &str) {
        let scheme = glib::Uri::peek_scheme(uri).unwrap_or_default();
        if !matches!(scheme.as_str(), "http" | "https" | "mailto") {
            return;
        }
        gtk::UriLauncher::new(uri).launch(Some(&self.window), gio::Cancellable::NONE, |result| {
            if let Err(error) = result {
                eprintln!("Couldn’t open a link: {error}");
            }
        });
    }
}

fn main_menu() -> gio::Menu {
    let view = gio::Menu::new();
    view.append(Some("_Fullscreen"), Some("win.fullscreen"));
    view.append(Some("_Reload"), Some("win.reload"));
    let app = gio::Menu::new();
    app.append(Some("_Preferences"), Some("win.preferences"));
    app.append(Some("_Keyboard Shortcuts"), Some("win.shortcuts"));
    app.append(Some("_About Mini Tokyo 3D"), Some("win.about"));
    let menu = gio::Menu::new();
    menu.append_section(None, &view);
    menu.append_section(None, &app);
    menu
}

fn create_web_view() -> webkit::WebView {
    let data_dir = glib::user_data_dir().join("mini-tokyo-3d").join("webkit");
    let cache_dir = glib::user_cache_dir().join("mini-tokyo-3d").join("webkit");
    let session = webkit::NetworkSession::new(data_dir.to_str(), cache_dir.to_str());

    let settings = webkit::Settings::new();
    settings.set_hardware_acceleration_policy(webkit::HardwareAccelerationPolicy::Always);
    settings.set_enable_webgl(true);
    settings.set_enable_webaudio(true);
    settings.set_enable_media(true);
    settings.set_enable_mediasource(true);
    // Live cameras start playing when they're opened, as on the website.
    settings.set_media_playback_requires_user_gesture(false);
    settings.set_javascript_can_open_windows_automatically(false);
    settings.set_enable_back_forward_navigation_gestures(false);
    settings.set_enable_developer_extras(cfg!(debug_assertions));
    settings.set_enable_write_console_messages_to_stdout(cfg!(debug_assertions));

    if let Some(context) = webkit::WebContext::default() {
        let languages = preferred_languages();
        let languages: Vec<&str> = languages.iter().map(String::as_str).collect();
        context.set_preferred_languages(&languages);
    }

    // Like browsers, let the live cameras play with sound once they're opened.
    let policies = webkit::WebsitePolicies::builder()
        .autoplay(webkit::AutoplayPolicy::Allow)
        .build();

    webkit::WebView::builder()
        .network_session(&session)
        .settings(&settings)
        .website_policies(&policies)
        .hexpand(true)
        .vexpand(true)
        .build()
}

/// The system's languages as BCP 47 tags (ja_JP.UTF-8 → ja-JP), which the map
/// uses to pick its language when none is set.
fn preferred_languages() -> Vec<String> {
    let mut languages = Vec::new();
    for name in glib::language_names() {
        if name.contains(['.', '@']) || name == "C" || name == "POSIX" {
            continue;
        }
        let tag = name.replace('_', "-");
        if !languages.contains(&tag) {
            languages.push(tag);
        }
    }
    if languages.is_empty() {
        languages.push("en".to_owned());
    }
    languages
}

fn token_page() -> (gtk::Widget, adw::PasswordEntryRow, gtk::Label, gtk::Button) {
    let entry = adw::PasswordEntryRow::builder().title("Mapbox Access Token").build();
    let group = adw::PreferencesGroup::new();
    group.add(&entry);

    let error = gtk::Label::builder()
        .wrap(true)
        .visible(false)
        .css_classes(["error"])
        .build();

    let button = gtk::Button::builder()
        .label("Continue")
        .halign(gtk::Align::Center)
        .sensitive(false)
        .css_classes(["pill", "suggested-action"])
        .build();

    let note = gtk::Label::builder()
        .label(
            "The token is kept in GNOME Keyring. Optional ODPT tokens, which add live positions for more trains, can be added later in Preferences.",
        )
        .wrap(true)
        .justify(gtk::Justification::Center)
        .css_classes(["dim-label", "caption"])
        .build();

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .build();
    content.append(&group);
    content.append(&error);
    content.append(&button);
    content.append(&note);

    let clamp = adw::Clamp::builder().maximum_size(420).child(&content).build();
    let page = adw::StatusPage::builder()
        .icon_name(crate::APP_ID)
        .title("Welcome to Mini Tokyo 3D")
        .description(
            "The 3D map is drawn by Mapbox, which needs an access token. Create one for free on your \
             <a href=\"https://account.mapbox.com/access-tokens/\">Mapbox account page</a> and paste it below.",
        )
        .child(&clamp)
        .build();
    page.add_css_class("icon-dropshadow");
    (page.upcast(), entry, error, button)
}

/// Directories to look in for the web app, in order.
fn web_dir_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(dir) = std::env::var_os("MT3D_WEB_DIR") {
        candidates.push(PathBuf::from(dir));
    }
    // Installed: <prefix>/bin/mini-tokyo-3d and <prefix>/share/mini-tokyo-3d/web
    if let Some(prefix) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent()?.parent().map(PathBuf::from))
    {
        candidates.push(prefix.join("share").join("mini-tokyo-3d").join("web"));
    }
    // Run from the source tree with `cargo run`
    candidates.push(PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/_build/web")));
    candidates
}

fn web_dir() -> Option<PathBuf> {
    web_dir_candidates()
        .into_iter()
        .find(|dir| dir.join("index.html").is_file())
}
