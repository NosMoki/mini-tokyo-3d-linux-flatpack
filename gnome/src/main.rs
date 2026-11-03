//! Mini Tokyo 3D for GNOME.
//!
//! A GTK 4 and libadwaita window around the Mini Tokyo 3D web app, which runs
//! unchanged in WebKitGTK, so the map, trains, buses, flights, live cameras and
//! plugins work exactly as they do on minitokyo3d.com.

mod dev;
mod preferences;
mod secrets;
mod server;
mod shortcuts;
mod window;

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};

use window::Window;

pub const APP_ID: &str = "io.github.nosmoki.MiniTokyo3D";
pub const APP_NAME: &str = "Mini Tokyo 3D";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Options from the command line, passed to the page as URL parameters.
#[derive(Clone, Debug, Default)]
pub struct LaunchOptions {
    pub lang: Option<String>,
    pub selection: Option<String>,
    pub gtfs_url: Option<String>,
    pub gtfs_vp_url: Option<String>,
    pub gtfs_color: Option<String>,
}

impl LaunchOptions {
    fn from_dict(dict: &glib::VariantDict) -> LaunchOptions {
        let get = |name: &str| {
            dict.lookup::<String>(name)
                .ok()
                .flatten()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        };
        LaunchOptions {
            lang: get("lang"),
            selection: get("selection"),
            gtfs_url: get("gtfs-url"),
            gtfs_vp_url: get("gtfs-vp-url"),
            gtfs_color: get("gtfs-color"),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.lang.is_none()
            && self.selection.is_none()
            && self.gtfs_url.is_none()
            && self.gtfs_vp_url.is_none()
            && self.gtfs_color.is_none()
    }
}

fn main() -> glib::ExitCode {
    glib::set_application_name(APP_NAME);

    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

    let string = glib::OptionArg::String;
    let none = glib::OptionFlags::NONE;
    let no_short = glib::Char::from(0);
    app.add_main_option(
        "lang",
        glib::Char::from(b'l'),
        none,
        string,
        "Language of the map, such as en or ja",
        Some("LANG"),
    );
    app.add_main_option(
        "selection",
        no_short,
        none,
        string,
        "Train, flight or station to select at startup",
        Some("ID"),
    );
    app.add_main_option(
        "gtfs-url",
        no_short,
        none,
        string,
        "Show only this GTFS feed (needs --gtfs-color)",
        Some("URL"),
    );
    app.add_main_option(
        "gtfs-vp-url",
        no_short,
        none,
        string,
        "GTFS Realtime vehicle positions for --gtfs-url",
        Some("URL"),
    );
    app.add_main_option(
        "gtfs-color",
        no_short,
        none,
        string,
        "Route color for --gtfs-url, such as 2E7D32",
        Some("RRGGBB"),
    );
    app.add_main_option(
        "version",
        no_short,
        none,
        glib::OptionArg::None,
        "Show the version",
        None,
    );

    app.connect_handle_local_options(|_, dict| {
        if dict.contains("version") {
            println!("{APP_NAME} {VERSION}");
            return std::ops::ControlFlow::Break(glib::ExitCode::SUCCESS);
        }
        std::ops::ControlFlow::Continue(())
    });

    app.connect_startup(|app| {
        setup_icons();
        gtk::Window::set_default_icon_name(APP_ID);
        app.set_accels_for_action("win.fullscreen", &["F11"]);
        app.set_accels_for_action("win.reload", &["<Control>r", "F5"]);
        app.set_accels_for_action("win.preferences", &["<Control>comma"]);
        app.set_accels_for_action("win.shortcuts", &["<Control>question"]);
        app.set_accels_for_action("window.close", &["<Control>w"]);
        app.set_accels_for_action("app.quit", &["<Control>q"]);
        let quit = gio::ActionEntry::builder("quit")
            .activate(|app: &adw::Application, _, _| app.quit())
            .build();
        app.add_action_entries([quit]);
    });

    let main_window: Rc<RefCell<Option<Rc<Window>>>> = Rc::default();
    app.connect_command_line(move |app, command_line| {
        let launch = LaunchOptions::from_dict(&command_line.options_dict());
        let existing = main_window.borrow().clone();
        let window = match existing {
            Some(window) => {
                window.apply_launch_options(launch);
                window
            }
            None => {
                let window = Window::new(app, load_settings(), launch);
                let slot = Rc::clone(&main_window);
                window.window.connect_destroy(move |_| {
                    slot.borrow_mut().take();
                });
                main_window.replace(Some(Rc::clone(&window)));
                window
            }
        };
        window.window.present();
        glib::ExitCode::SUCCESS
    });

    app.run()
}

/// Opens the app's GSettings, from the installed schema or, when run from the
/// source tree, from the one compiled by build.rs.
fn load_settings() -> gio::Settings {
    let default_source = gio::SettingsSchemaSource::default();
    let installed = default_source.as_ref().and_then(|source| source.lookup(APP_ID, true));
    let schema = installed.or_else(|| {
        gio::SettingsSchemaSource::from_directory(env!("MT3D_SCHEMA_DIR"), default_source.as_ref(), false)
            .ok()?
            .lookup(APP_ID, false)
    });
    match schema {
        Some(schema) => gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None),
        None => panic!("The GSettings schema {APP_ID} is not installed"),
    }
}

/// Lets the app find its icon when run from the source tree.
fn setup_icons() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let theme = gtk::IconTheme::for_display(&display);
    if !theme.has_icon(APP_ID) {
        theme.add_search_path(concat!(env!("CARGO_MANIFEST_DIR"), "/data/icons"));
    }
}

pub fn present_about(parent: &impl IsA<gtk::Widget>) {
    let about = adw::AboutDialog::builder()
        .application_name(APP_NAME)
        .application_icon(APP_ID)
        .developer_name("Akihiko Kusanagi")
        .version(VERSION)
        .comments("A real-time 3D digital map of Tokyo’s public transport system")
        .website("https://minitokyo3d.com")
        .issue_url("https://github.com/NosMoki/mini-tokyo-3d-rust-converting/issues")
        .copyright("© 2019-2026 Akihiko Kusanagi")
        .license_type(gtk::License::MitX11)
        .developers(["Akihiko Kusanagi https://github.com/nagix"])
        .build();
    about.add_link("User Guide", "https://minitokyo3d.com/user-guide/");
    about.add_credit_section(Some("GNOME App"), &["NosMoki https://github.com/NosMoki"]);
    about.add_legal_section(
        "Map and Data",
        None,
        gtk::License::Custom,
        Some(
            "The base map is © Mapbox and © OpenStreetMap contributors. Transit data comes from the \
             Public Transportation Open Data Center (ODPT) and the operators. The map’s own About \
             panel lists every source.",
        ),
    );
    about.present(Some(parent));
}
