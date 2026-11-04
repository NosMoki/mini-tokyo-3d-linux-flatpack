/* eslint-disable import-x/no-unresolved -- gi:// modules are resolved by GJS */
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import WebKit from 'gi://WebKit?version=6.0';

// The web bundle is served to the web view from the install directory through
// a private URI scheme, so the page gets a proper secure origin (needed by
// fetch(), workers and the Cache API) without running a local web server.
export const SCHEME = 'mt3d';
export const APP_URI = `${SCHEME}://app/`;

const MIME_TYPES = {
    css: 'text/css',
    html: 'text/html',
    jpg: 'image/jpeg',
    js: 'text/javascript',
    json: 'application/json',
    map: 'application/json',
    png: 'image/png',
    svg: 'image/svg+xml',
    wasm: 'application/wasm'
};

Gio._promisify(Gio.File.prototype, 'query_info_async', 'query_info_finish');
Gio._promisify(Gio.File.prototype, 'read_async', 'read_finish');

let registered = false;

export function isAppUri(uri) {
    return typeof uri === 'string' && uri.startsWith(`${SCHEME}:`);
}

function getMimeType(path, info) {
    const extension = path.slice(path.lastIndexOf('.') + 1).toLowerCase();

    return MIME_TYPES[extension] || Gio.content_type_get_mime_type(info.get_content_type()) || 'application/octet-stream';
}

async function serve(root, request) {
    const path = GLib.Uri.unescape_string(request.get_path() || '/', '/');

    if (path === null) {
        throw new GLib.Error(Gio.IOErrorEnum, Gio.IOErrorEnum.INVALID_ARGUMENT, `Invalid path: ${request.get_uri()}`);
    }

    const file = root.resolve_relative_path(path.replace(/^\/+/, '') || 'index.html');

    if (!file.has_prefix(root)) {
        throw new GLib.Error(Gio.IOErrorEnum, Gio.IOErrorEnum.PERMISSION_DENIED, `Access denied: ${request.get_uri()}`);
    }

    const info = await file.query_info_async('standard::type,standard::size,standard::content-type',
        Gio.FileQueryInfoFlags.NONE, GLib.PRIORITY_DEFAULT, null);

    if (info.get_file_type() !== Gio.FileType.REGULAR) {
        throw new GLib.Error(Gio.IOErrorEnum, Gio.IOErrorEnum.NOT_FOUND, `Not found: ${request.get_uri()}`);
    }

    const stream = await file.read_async(GLib.PRIORITY_DEFAULT, null),
        response = WebKit.URISchemeResponse.new(stream, info.get_size());

    response.set_status(200, null);
    response.set_content_type(getMimeType(file.get_path(), info));
    request.finish_with_response(response);
}

export function registerAppScheme(context, webDir) {
    if (registered) {
        return;
    }
    registered = true;

    const root = Gio.File.new_for_path(webDir),
        security = context.get_security_manager();

    context.register_uri_scheme(SCHEME, request => {
        serve(root, request).catch(error => {
            request.finish_error(error instanceof GLib.Error ? error :
                new GLib.Error(Gio.IOErrorEnum, Gio.IOErrorEnum.FAILED, String(error)));
        });
    });
    security.register_uri_scheme_as_secure(SCHEME);
    security.register_uri_scheme_as_cors_enabled(SCHEME);
}
