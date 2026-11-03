//! A small HTTP server that serves the web app to the WebView.
//!
//! The page is served over http://127.0.0.1 rather than a custom URI scheme so
//! that it behaves exactly as it does in a browser: WebKit only sends a Referer
//! from http(s) pages, and the YouTube player used by the Live Camera plugin
//! refuses to play without one.
//!
//! The server listens on loopback only, answers only requests addressed to its
//! own host and port, and serves everything under a random path prefix, so
//! other programs can't guess where the page is. The access tokens are filled
//! into index.html as it is served, so they never touch the disk.

use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

/// Placeholders written into index.html by gnome/build-web.mjs.
const MAPBOX_PLACEHOLDER: &str = "\"MT3D_DESKTOP_MAPBOX_ACCESS_TOKEN\"";
const ODPT_PLACEHOLDER: &str = "\"MT3D_DESKTOP_ODPT_ACCESS_TOKEN\"";
const CHALLENGE_PLACEHOLDER: &str = "\"MT3D_DESKTOP_CHALLENGE_ACCESS_TOKEN\"";

const MAX_HEADER_BYTES: u64 = 16 * 1024;
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// Access tokens passed to the page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tokens {
    pub mapbox: String,
    pub odpt: String,
    pub challenge: String,
}

pub struct Server {
    port: u16,
    shared: Arc<Shared>,
}

struct Shared {
    root: PathBuf,
    prefix: String,
    host: String,
    tokens: RwLock<Tokens>,
}

impl Server {
    /// Starts serving `root` on an unused loopback port.
    pub fn start(root: &Path) -> io::Result<Server> {
        let root = root.canonicalize()?;
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let port = listener.local_addr()?.port();
        let shared = Arc::new(Shared {
            root,
            prefix: random_hex(16)?,
            host: format!("127.0.0.1:{port}"),
            tokens: RwLock::default(),
        });

        let accept_shared = Arc::clone(&shared);
        thread::Builder::new().name("mt3d-http".into()).spawn(move || {
            for stream in listener.incoming().flatten() {
                let shared = Arc::clone(&accept_shared);
                let _ = thread::Builder::new().name("mt3d-http-conn".into()).spawn(move || {
                    let _ = serve_connection(stream, &shared);
                });
            }
        })?;

        Ok(Server { port, shared })
    }

    /// The URL of the web app's directory, ending with a slash.
    pub fn base_uri(&self) -> String {
        format!("http://127.0.0.1:{}/{}/", self.port, self.shared.prefix)
    }

    pub fn tokens(&self) -> Tokens {
        self.shared.tokens.read().unwrap().clone()
    }

    pub fn set_tokens(&self, tokens: Tokens) {
        *self.shared.tokens.write().unwrap() = tokens;
    }
}

struct Request {
    method: String,
    target: String,
    host: Option<String>,
    keep_alive: bool,
}

fn serve_connection(stream: TcpStream, shared: &Shared) -> io::Result<()> {
    stream.set_read_timeout(Some(IDLE_TIMEOUT))?;
    stream.set_nodelay(true)?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;

    while let Some(request) = read_request(&mut reader)? {
        let keep_alive = request.keep_alive;
        respond(&mut writer, &request, shared)?;
        if !keep_alive {
            break;
        }
    }
    let _ = writer.shutdown(Shutdown::Both);
    Ok(())
}

fn read_request(reader: &mut BufReader<TcpStream>) -> io::Result<Option<Request>> {
    let mut head = reader.by_ref().take(MAX_HEADER_BYTES);
    let mut line = String::new();
    if head.read_line(&mut line)? == 0 {
        return Ok(None);
    }

    let mut parts = line.split_ascii_whitespace();
    let (Some(method), Some(target), Some(version)) = (parts.next(), parts.next(), parts.next()) else {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "bad request line"));
    };
    let mut request = Request {
        method: method.to_owned(),
        target: target.to_owned(),
        host: None,
        keep_alive: version == "HTTP/1.1",
    };

    loop {
        line.clear();
        if head.read_line(&mut line)? == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "truncated headers"));
        }
        let header = line.trim_end_matches(['\r', '\n']);
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            let value = value.trim();
            if name.eq_ignore_ascii_case("host") {
                request.host = Some(value.to_owned());
            } else if name.eq_ignore_ascii_case("connection") {
                if value.eq_ignore_ascii_case("close") {
                    request.keep_alive = false;
                } else if value.eq_ignore_ascii_case("keep-alive") {
                    request.keep_alive = true;
                }
            } else if name.eq_ignore_ascii_case("content-length") && value != "0" {
                // Only GET and HEAD are served, and they don't have bodies.
                request.keep_alive = false;
            }
        }
    }
    Ok(Some(request))
}

fn respond(writer: &mut TcpStream, request: &Request, shared: &Shared) -> io::Result<()> {
    let head_only = request.method == "HEAD";
    if !head_only && request.method != "GET" {
        return send_error(writer, request, 405, "Method Not Allowed");
    }
    if request.host.as_deref() != Some(shared.host.as_str()) {
        return send_error(writer, request, 421, "Misdirected Request");
    }
    let Some(relative) = resolve(&request.target, &shared.prefix) else {
        return send_error(writer, request, 404, "Not Found");
    };
    let path = shared.root.join(&relative);
    let is_file = path
        .canonicalize()
        .map(|real| real.starts_with(&shared.root) && real.is_file())
        .unwrap_or(false);
    if !is_file {
        return send_error(writer, request, 404, "Not Found");
    }

    let body = if relative == Path::new("index.html") {
        let html = fs::read_to_string(&path)?;
        fill_tokens(&html, &shared.tokens.read().unwrap()).into_bytes()
    } else {
        fs::read(&path)?
    };

    let header = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Connection: {}\r\n\r\n",
        content_type(&path),
        body.len(),
        if request.keep_alive { "keep-alive" } else { "close" },
    );
    writer.write_all(header.as_bytes())?;
    if !head_only {
        writer.write_all(&body)?;
    }
    writer.flush()
}

fn send_error(writer: &mut TcpStream, request: &Request, status: u16, reason: &str) -> io::Result<()> {
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Connection: {}\r\n\r\n",
        reason.len(),
        if request.keep_alive { "keep-alive" } else { "close" },
    );
    writer.write_all(header.as_bytes())?;
    if request.method != "HEAD" {
        writer.write_all(reason.as_bytes())?;
    }
    writer.flush()
}

/// Maps a request target like `/<prefix>/assets/style.json?x=1` to a relative
/// file path, or `None` if it is outside the prefix or not a plain path.
fn resolve(target: &str, prefix: &str) -> Option<PathBuf> {
    let path = target.split(['?', '#']).next()?;
    let rest = path.strip_prefix('/')?.strip_prefix(prefix)?.strip_prefix('/')?;
    let decoded = percent_decode(rest)?;
    if decoded.is_empty() {
        return Some(PathBuf::from("index.html"));
    }

    let mut relative = PathBuf::new();
    for segment in decoded.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." || segment.contains(['\\', '\0']) {
            return None;
        }
        relative.push(segment);
    }
    Some(relative)
}

fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn fill_tokens(html: &str, tokens: &Tokens) -> String {
    html.replace(MAPBOX_PLACEHOLDER, &js_string(&tokens.mapbox))
        .replace(ODPT_PLACEHOLDER, &js_string(&tokens.odpt))
        .replace(CHALLENGE_PLACEHOLDER, &js_string(&tokens.challenge))
}

/// Quotes a string for a JavaScript string literal inside an HTML <script>.
fn js_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '&' => out.push_str("\\u0026"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn content_type(path: &Path) -> &'static str {
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match extension.to_ascii_lowercase().as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "webmanifest" => "application/manifest+json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "ico" => "image/vnd.microsoft.icon",
        "wasm" => "application/wasm",
        "glb" => "model/gltf-binary",
        "pbf" => "application/x-protobuf",
        "gz" => "application/gzip",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn random_hex(bytes: usize) -> io::Result<String> {
    let mut buffer = vec![0; bytes];
    fs::File::open("/dev/urandom")?.read_exact(&mut buffer)?;
    Ok(buffer.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_paths_under_the_prefix() {
        let prefix = "abc";
        assert_eq!(resolve("/abc/", prefix), Some(PathBuf::from("index.html")));
        assert_eq!(
            resolve("/abc/index.html?lang=ja#12/35/139", prefix),
            Some(PathBuf::from("index.html"))
        );
        assert_eq!(
            resolve("/abc/assets/style.json", prefix),
            Some(PathBuf::from("assets/style.json"))
        );
        assert_eq!(resolve("/abc/a%20b.js", prefix), Some(PathBuf::from("a b.js")));
        assert_eq!(resolve("/abc", prefix), None);
        assert_eq!(resolve("/abcd/index.html", prefix), None);
        assert_eq!(resolve("/index.html", prefix), None);
        assert_eq!(resolve("/abc/../secret", prefix), None);
        assert_eq!(resolve("/abc/%2e%2e/secret", prefix), None);
        assert_eq!(resolve("/abc/assets//style.json", prefix), None);
        assert_eq!(resolve("/abc/a%5cb", prefix), None);
        assert_eq!(resolve("/abc/%zz", prefix), None);
    }

    #[test]
    fn fills_tokens_as_safe_string_literals() {
        let html = "a = \"MT3D_DESKTOP_MAPBOX_ACCESS_TOKEN\"; b = {odpt: \"MT3D_DESKTOP_ODPT_ACCESS_TOKEN\", challenge: \"MT3D_DESKTOP_CHALLENGE_ACCESS_TOKEN\"};";
        let tokens = Tokens {
            mapbox: "pk.abc".into(),
            odpt: String::new(),
            challenge: "x\"</script>".into(),
        };
        assert_eq!(
            fill_tokens(html, &tokens),
            "a = \"pk.abc\"; b = {odpt: \"\", challenge: \"x\\\"\\u003c/script\\u003e\"};"
        );
    }
}
