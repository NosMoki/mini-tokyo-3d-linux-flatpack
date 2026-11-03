//! Access tokens kept in GNOME Keyring (through the Secret portal when the app
//! runs as a Flatpak), and checks of the Mapbox token.

use std::collections::HashMap;

use gtk::glib;
use soup::prelude::*;

use crate::APP_ID;
use crate::server::Tokens;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Secret {
    Mapbox,
    Odpt,
    Challenge,
}

impl Secret {
    pub const ALL: [Secret; 3] = [Secret::Mapbox, Secret::Odpt, Secret::Challenge];

    fn kind(self) -> &'static str {
        match self {
            Secret::Mapbox => "mapbox-access-token",
            Secret::Odpt => "odpt-access-token",
            Secret::Challenge => "odpt-challenge-access-token",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Secret::Mapbox => "Mini Tokyo 3D: Mapbox access token",
            Secret::Odpt => "Mini Tokyo 3D: ODPT access token",
            Secret::Challenge => "Mini Tokyo 3D: ODPT Challenge access token",
        }
    }

    pub fn get(self, tokens: &Tokens) -> &str {
        match self {
            Secret::Mapbox => &tokens.mapbox,
            Secret::Odpt => &tokens.odpt,
            Secret::Challenge => &tokens.challenge,
        }
    }

    pub fn set(self, tokens: &mut Tokens, value: String) {
        match self {
            Secret::Mapbox => tokens.mapbox = value,
            Secret::Odpt => tokens.odpt = value,
            Secret::Challenge => tokens.challenge = value,
        }
    }
}

fn schema() -> libsecret::Schema {
    let mut attributes = HashMap::new();
    attributes.insert("kind", libsecret::SchemaAttributeType::String);
    libsecret::Schema::new(APP_ID, libsecret::SchemaFlags::NONE, attributes)
}

fn attributes(secret: Secret) -> HashMap<&'static str, &'static str> {
    HashMap::from([("kind", secret.kind())])
}

/// Reads all tokens. Missing ones are left empty.
pub async fn load_all() -> Result<Tokens, glib::Error> {
    let schema = schema();
    let mut tokens = Tokens::default();
    for secret in Secret::ALL {
        let value = libsecret::password_lookup_future(Some(&schema), attributes(secret)).await?;
        secret.set(&mut tokens, value.map(String::from).unwrap_or_default());
    }
    Ok(tokens)
}

/// Saves a token, or removes it when `value` is empty.
pub async fn store(secret: Secret, value: &str) -> Result<(), glib::Error> {
    let schema = schema();
    if value.is_empty() {
        libsecret::password_clear_future(Some(&schema), attributes(secret)).await
    } else {
        libsecret::password_store_future(
            Some(&schema),
            attributes(secret),
            Some(libsecret::COLLECTION_DEFAULT.as_str()),
            secret.label(),
            value,
        )
        .await
    }
}

/// Characters that appear in Mapbox and ODPT tokens.
fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')
}

/// Checks the form of an optional ODPT token.
pub fn check_odpt_token(token: &str) -> Result<(), &'static str> {
    if token.chars().all(is_token_char) {
        Ok(())
    } else {
        Err("That doesn’t look like an access token")
    }
}

/// Checks the form of a Mapbox token without going online.
pub fn check_mapbox_token(token: &str) -> Result<(), &'static str> {
    if token.is_empty() {
        Err("Enter a Mapbox access token")
    } else if !token.chars().all(is_token_char) {
        Err("That doesn’t look like a Mapbox access token")
    } else if token.starts_with("sk.") {
        Err("This is a secret token. Use a public token, which starts with “pk.”")
    } else if !token.starts_with("pk.") {
        Err("Mapbox public tokens start with “pk.”")
    } else {
        Ok(())
    }
}

pub enum Verdict {
    Valid,
    /// Mapbox rejected it, with the reason Mapbox gave.
    Invalid(String),
    /// Mapbox couldn't be reached, so the token wasn't checked.
    Unchecked,
}

/// Asks Mapbox whether a token is valid.
pub async fn verify_mapbox_token(token: &str) -> Verdict {
    let session = soup::Session::new();
    session.set_timeout(15);
    let uri = format!("https://api.mapbox.com/tokens/v2?access_token={token}");
    let Ok(message) = soup::Message::new("GET", &uri) else {
        return Verdict::Invalid("TokenMalformed".into());
    };
    let Ok(body) = session.send_and_read_future(&message, glib::Priority::DEFAULT).await else {
        return Verdict::Unchecked;
    };
    let body = String::from_utf8_lossy(&body);
    match json_string_field(&body, "code") {
        Some("TokenValid") => Verdict::Valid,
        Some(code) => Verdict::Invalid(code.to_owned()),
        None if message.status() == soup::Status::Unauthorized => Verdict::Invalid("TokenInvalid".into()),
        None => Verdict::Unchecked,
    }
}

/// Explains a Mapbox token check result code.
pub fn describe_mapbox_code(code: &str) -> &'static str {
    match code {
        "TokenExpired" => "Mapbox says this token has expired",
        "TokenRevoked" => "Mapbox says this token has been revoked",
        "TokenMalformed" => "That doesn’t look like a Mapbox access token",
        _ => "Mapbox didn’t accept this token",
    }
}

/// Finds `"name": "value"` in a flat JSON object.
fn json_string_field<'a>(json: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("\"{name}\"");
    let after_key = &json[json.find(&key)? + key.len()..];
    let after_colon = after_key.trim_start().strip_prefix(':')?.trim_start();
    let value = after_colon.strip_prefix('"')?;
    Some(&value[..value.find('"')?])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_mapbox_token_form() {
        assert!(check_mapbox_token("pk.eyJ1IjoiYSJ9.abc-_").is_ok());
        assert!(check_mapbox_token("").is_err());
        assert!(check_mapbox_token("sk.eyJ1").is_err());
        assert!(check_mapbox_token("eyJ1").is_err());
        assert!(check_mapbox_token("pk.a b").is_err());
    }

    #[test]
    fn reads_the_code_field() {
        assert_eq!(
            json_string_field(r#"{"code":"TokenValid","token":{}}"#, "code"),
            Some("TokenValid")
        );
        assert_eq!(
            json_string_field(r#"{ "code" : "TokenInvalid" }"#, "code"),
            Some("TokenInvalid")
        );
        assert_eq!(json_string_field(r#"{"message":"x"}"#, "code"), None);
    }
}
