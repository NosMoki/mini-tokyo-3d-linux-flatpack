//! Hooks for testing debug builds without a person at the screen. They read
//! environment variables and do nothing in release builds.
//!
//! - `MT3D_DEV_MAPBOX_TOKEN`, `MT3D_DEV_ODPT_TOKEN`, `MT3D_DEV_CHALLENGE_TOKEN`:
//!   use these tokens instead of the keyring.
//! - `MT3D_DEV_SCRIPT=file.js`: run a script in the page when it has loaded.
//! - `MT3D_DEV_SCREENSHOTS=out.png` and `MT3D_DEV_SCREENSHOT_DELAYS=10,30`:
//!   save the window to `out-10.png` and `out-30.png` that many seconds after
//!   it opens, then quit.
//! - `MT3D_DEV_ACTION=win.preferences`: activate an action 3 seconds after the
//!   window opens.

use std::rc::Rc;

use crate::server::Tokens;
use crate::window::Window;

#[cfg(not(debug_assertions))]
pub fn attach(_window: &Rc<Window>) {}

#[cfg(not(debug_assertions))]
pub fn token_override() -> Option<Tokens> {
    None
}

#[cfg(debug_assertions)]
pub fn token_override() -> Option<Tokens> {
    let mapbox = std::env::var("MT3D_DEV_MAPBOX_TOKEN").ok()?;
    Some(Tokens {
        mapbox,
        odpt: std::env::var("MT3D_DEV_ODPT_TOKEN").unwrap_or_default(),
        challenge: std::env::var("MT3D_DEV_CHALLENGE_TOKEN").unwrap_or_default(),
    })
}

#[cfg(debug_assertions)]
pub fn attach(window: &Rc<Window>) {
    use adw::prelude::*;
    use gtk::{gio, glib};
    use webkit::prelude::*;

    if let Ok(path) = std::env::var("MT3D_DEV_SCRIPT") {
        let script = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"));
        window.web_view.connect_load_changed(move |view, event| {
            if event == webkit::LoadEvent::Finished {
                view.evaluate_javascript(&script, None, None, gio::Cancellable::NONE, |result| match result {
                    Ok(value) => eprintln!("[dev] script: {}", value.to_str()),
                    Err(error) => eprintln!("[dev] script failed: {error}"),
                });
            }
        });
    }

    if let Ok(action) = std::env::var("MT3D_DEV_ACTION") {
        let weak = Rc::downgrade(window);
        glib::timeout_add_seconds_local_once(3, move || {
            if let Some(window) = weak.upgrade()
                && let Err(error) = WidgetExt::activate_action(&window.window, &action, None)
            {
                eprintln!("[dev] couldn’t activate {action}: {error}");
            }
        });
    }

    let Ok(output) = std::env::var("MT3D_DEV_SCREENSHOTS") else {
        return;
    };
    let delays: Vec<u32> = std::env::var("MT3D_DEV_SCREENSHOT_DELAYS")
        .unwrap_or_else(|_| "20".into())
        .split(',')
        .filter_map(|delay| delay.trim().parse().ok())
        .collect();
    let last = delays.iter().copied().max().unwrap_or(0);
    let stem = output.trim_end_matches(".png").to_owned();
    for delay in delays {
        let weak = Rc::downgrade(window);
        let path = format!("{stem}-{delay}.png");
        glib::timeout_add_seconds_local_once(delay, move || {
            let Some(window) = weak.upgrade() else {
                return;
            };
            match save_screenshot(&window.window, &path) {
                Ok(()) => eprintln!("[dev] saved {path}"),
                Err(error) => eprintln!("[dev] couldn’t save {path}: {error}"),
            }
            if delay == last
                && let Some(app) = window.window.application()
            {
                app.quit();
            }
        });
    }
}

#[cfg(debug_assertions)]
fn save_screenshot(window: &adw::ApplicationWindow, path: &str) -> Result<(), String> {
    use adw::prelude::*;

    // The window itself, so that its background is included
    let (width, height) = (window.width() as f64, window.height() as f64);
    let paintable = gtk::WidgetPaintable::new(Some(window));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, width, height);
    let node = snapshot.to_node().ok_or("nothing drawn")?;
    let renderer = window.renderer().ok_or("no renderer")?;
    let bounds = gtk::graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
    let texture = renderer.render_texture(&node, Some(&bounds));
    texture.save_to_png(path).map_err(|error| error.to_string())
}
