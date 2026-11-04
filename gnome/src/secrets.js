/* eslint-disable import-x/no-unresolved -- gi:// modules are resolved by GJS */
import Gio from 'gi://Gio';
import Secret from 'gi://Secret';

// Access tokens are kept in the user's keyring (GNOME Keyring, through the
// Secret Service API) rather than in a settings file or in the app itself.
const SCHEMA = new Secret.Schema('com.minitokyo3d.MiniTokyo3D.Token', Secret.SchemaFlags.NONE, {
    name: Secret.SchemaAttributeType.STRING
});

// Names of the stored tokens: the Mapbox access token, and the ODPT Center and
// Open Data Challenge tokens that the web build takes as MT3D_SECRET_ODPT and
// MT3D_SECRET_CHALLENGE.
export const TOKEN_NAMES = ['mapbox', 'odpt', 'challenge'];

Gio._promisify(Secret, 'password_lookup', 'password_lookup_finish');
Gio._promisify(Secret, 'password_store', 'password_store_finish');
Gio._promisify(Secret, 'password_clear', 'password_clear_finish');

export async function lookupTokens() {
    const values = await Promise.all(TOKEN_NAMES.map(name =>
        Secret.password_lookup(SCHEMA, {name}, null)));

    return Object.fromEntries(TOKEN_NAMES.map((name, i) => [name, values[i] || '']));
}

export async function storeToken(name, label, value) {
    if (value) {
        await Secret.password_store(SCHEMA, {name}, Secret.COLLECTION_DEFAULT, label, value, null);
    } else {
        await Secret.password_clear(SCHEMA, {name}, null);
    }
}
