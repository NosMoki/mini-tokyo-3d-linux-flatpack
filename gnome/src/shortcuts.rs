//! The keyboard shortcuts dialog: the app's own shortcuts and the map's.

use adw::prelude::*;

pub fn present(parent: &impl IsA<gtk::Widget>) {
    let dialog = adw::ShortcutsDialog::new();

    let general = adw::ShortcutsSection::new(Some("General"));
    general.add(adw::ShortcutsItem::from_action("Fullscreen", "win.fullscreen"));
    general.add(adw::ShortcutsItem::from_action("Reload", "win.reload"));
    general.add(adw::ShortcutsItem::from_action("Preferences", "win.preferences"));
    general.add(adw::ShortcutsItem::from_action("Keyboard Shortcuts", "win.shortcuts"));
    general.add(adw::ShortcutsItem::from_action("Close Window", "window.close"));
    general.add(adw::ShortcutsItem::from_action("Quit", "app.quit"));
    dialog.add(general);

    // Mapbox GL's keyboard handler, active while the map has the focus
    let map = adw::ShortcutsSection::new(Some("Map"));
    map.add(adw::ShortcutsItem::new("Move", "Up Down Left Right"));
    map.add(adw::ShortcutsItem::new("Rotate", "<Shift>Left <Shift>Right"));
    map.add(adw::ShortcutsItem::new("Tilt", "<Shift>Up <Shift>Down"));
    map.add(adw::ShortcutsItem::new("Zoom In", "plus equal"));
    map.add(adw::ShortcutsItem::new("Zoom Out", "minus"));
    map.add(adw::ShortcutsItem::new("Zoom In Faster", "<Shift>plus"));
    map.add(adw::ShortcutsItem::new("Zoom Out Faster", "<Shift>minus"));
    dialog.add(map);

    dialog.present(Some(parent));
}
