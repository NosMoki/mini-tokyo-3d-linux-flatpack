//! The preferences dialog: the map's language and the access tokens.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use crate::secrets::{self, Secret, Verdict};
use crate::window::Window;

/// The languages of the web app (configs.langs), with their own names.
const LANGUAGES: [(&str, &str); 11] = [
    ("de", "Deutsch"),
    ("en", "English"),
    ("es", "Español"),
    ("fr", "Français"),
    ("ja", "日本語"),
    ("ko", "한국어"),
    ("ne", "नेपाली"),
    ("pt-BR", "Português (Brasil)"),
    ("th", "ไทย"),
    ("zh-Hans", "简体中文"),
    ("zh-Hant", "繁體中文"),
];

pub fn present(window: &Rc<Window>) {
    let dialog = adw::PreferencesDialog::new();
    dialog.set_search_enabled(false);
    let page = adw::PreferencesPage::new();
    dialog.add(&page);

    page.add(&language_group(window));
    page.add(&tokens_group(window, &dialog));

    dialog.present(Some(&window.window));
}

fn language_group(window: &Rc<Window>) -> adw::PreferencesGroup {
    let mut names = vec!["Automatic"];
    names.extend(LANGUAGES.iter().map(|(_, name)| *name));
    let row = adw::ComboRow::builder()
        .title("Language")
        .subtitle("Names and labels on the map")
        .model(&gtk::StringList::new(&names))
        .build();

    let current = window.settings.string("language");
    let selected = LANGUAGES
        .iter()
        .position(|(code, _)| *code == current.as_str())
        .map_or(0, |index| index + 1);
    row.set_selected(selected as u32);

    row.connect_selected_notify(glib::clone!(
        #[weak]
        window,
        move |row| {
            let code = match row.selected() {
                0 => "",
                index => LANGUAGES.get(index as usize - 1).map_or("", |(code, _)| *code),
            };
            if window.settings.string("language") != code {
                let _ = window.settings.set_string("language", code);
                window.load_map();
            }
        }
    ));

    let group = adw::PreferencesGroup::builder().title("Map").build();
    group.add(&row);
    group
}

fn tokens_group(window: &Rc<Window>, dialog: &adw::PreferencesDialog) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Access Tokens")
        .description(
            "Kept in GNOME Keyring. The <a href=\"https://account.mapbox.com/access-tokens/\">Mapbox token</a> \
             is needed for the map. The <a href=\"https://developer.odpt.org/\">ODPT tokens</a> are optional \
             and add live positions and service updates for more operators, such as Toei, Keikyu and Tobu.",
        )
        .build();

    let tokens = window.tokens();
    for (secret, title) in [
        (Secret::Mapbox, "Mapbox"),
        (Secret::Odpt, "ODPT"),
        (Secret::Challenge, "ODPT Challenge"),
    ] {
        let row = adw::PasswordEntryRow::builder()
            .title(title)
            .text(secret.get(&tokens))
            .show_apply_button(true)
            .build();
        row.connect_changed(|row| row.remove_css_class("error"));
        row.connect_apply(glib::clone!(
            #[weak]
            window,
            #[weak]
            dialog,
            move |row| apply_token(&window, &dialog, row, secret)
        ));
        group.add(&row);
    }
    group
}

fn apply_token(window: &Rc<Window>, dialog: &adw::PreferencesDialog, row: &adw::PasswordEntryRow, secret: Secret) {
    let value = row.text().trim().to_owned();
    let check = match secret {
        Secret::Mapbox => secrets::check_mapbox_token(&value),
        Secret::Odpt | Secret::Challenge => secrets::check_odpt_token(&value),
    };
    if let Err(message) = check {
        row.add_css_class("error");
        dialog.add_toast(adw::Toast::new(message));
        return;
    }

    let window = Rc::clone(window);
    let dialog = dialog.clone();
    let row = row.clone();
    row.set_sensitive(false);
    glib::spawn_future_local(async move {
        if secret == Secret::Mapbox {
            match secrets::verify_mapbox_token(&value).await {
                Verdict::Valid => {}
                Verdict::Invalid(code) => {
                    row.set_sensitive(true);
                    row.add_css_class("error");
                    dialog.add_toast(adw::Toast::new(secrets::describe_mapbox_code(&code)));
                    return;
                }
                Verdict::Unchecked => dialog.add_toast(adw::Toast::new("Couldn’t reach Mapbox to check the token")),
            }
        }
        row.set_sensitive(true);
        match window.save_token(secret, &value).await {
            Ok(()) if value.is_empty() => dialog.add_toast(adw::Toast::new("Token removed")),
            Ok(()) => dialog.add_toast(adw::Toast::new("Token saved")),
            Err(error) => {
                eprintln!("Couldn’t save the token: {error}");
                dialog.add_toast(adw::Toast::new("Couldn’t save the token in GNOME Keyring"));
            }
        }
        window.load_map();
    });
}
