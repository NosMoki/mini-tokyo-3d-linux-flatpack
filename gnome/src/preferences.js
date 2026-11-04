/* eslint-disable import-x/no-unresolved -- gi:// and GJS built-in modules are resolved by GJS */
import Adw from 'gi://Adw?version=1';
import GObject from 'gi://GObject';
import Gtk from 'gi://Gtk?version=4.0';
import Gettext from 'gettext';

import {lookupTokens, storeToken} from './secrets.js';

const _ = Gettext.gettext;

// Languages the map supports (configs.langs in the web app), each in its own name
const LANGUAGES = [
    ['de', 'Deutsch'],
    ['en', 'English'],
    ['es', 'Español'],
    ['fr', 'Français'],
    ['ja', '日本語'],
    ['ko', '한국어'],
    ['ne', 'नेपाली'],
    ['pt-BR', 'Português (Brasil)'],
    ['th', 'ไทย'],
    ['zh-Hans', '简体中文'],
    ['zh-Hant', '繁體中文']
];

export const MiniTokyo3DPreferencesDialog = GObject.registerClass({
    GTypeName: 'MiniTokyo3DPreferencesDialog',
    Template: 'resource:///com/minitokyo3d/MiniTokyo3D/preferences.ui',
    InternalChildren: ['languageRow', 'mapboxRow', 'odptRow', 'challengeRow'],
    Signals: {
        // Emitted once the dialog has closed, if any setting was changed
        'settings-changed': {}
    }
}, class MiniTokyo3DPreferencesDialog extends Adw.PreferencesDialog {

    constructor(settings) {
        super();

        const codes = ['', ...LANGUAGES.map(([code]) => code)];

        this._changed = false;
        this._saved = new Map();
        this._tokens = [
            {name: 'mapbox', row: this._mapboxRow, label: _('Mini Tokyo 3D: Mapbox access token')},
            {name: 'odpt', row: this._odptRow, label: _('Mini Tokyo 3D: ODPT Center token')},
            {name: 'challenge', row: this._challengeRow, label: _('Mini Tokyo 3D: Open Data Challenge token')}
        ];

        this._languageRow.model = Gtk.StringList.new([_('Automatic'), ...LANGUAGES.map(([, name]) => name)]);
        this._languageRow.selected = Math.max(codes.indexOf(settings.get_string('language')), 0);
        this._languageRow.connect('notify::selected', () => {
            settings.set_string('language', codes[this._languageRow.selected]);
            this._changed = true;
        });

        for (const token of this._tokens) {
            token.row.connect('apply', () => this._applyToken(token));
        }

        this.connect('closed', () => this._onClosed());

        this._loadTokens();
    }

    async _loadTokens() {
        let values;

        try {
            values = await lookupTokens();
        } catch (error) {
            console.error(error);
            this.add_toast(new Adw.Toast({title: _('Could not read the access tokens from the keyring')}));
            return;
        }

        for (const token of this._tokens) {
            const row = token.row;

            this._saved.set(token.name, values[token.name]);

            // Filling in the stored value must not count as an edit to apply.
            row.showApplyButton = false;
            row.text = values[token.name];
            row.showApplyButton = true;
            row.sensitive = true;
        }
    }

    // Stores the row's token if it was edited; resolves to the stored value.
    async _saveToken(token) {
        const value = token.row.text.trim();

        if (value !== this._saved.get(token.name)) {
            await storeToken(token.name, token.label, value);
            this._saved.set(token.name, value);
            this._changed = true;
        }
        return value;
    }

    async _applyToken(token) {
        try {
            const value = await this._saveToken(token);

            this.add_toast(new Adw.Toast({title: value ? _('Saved to keyring') : _('Removed from keyring')}));
        } catch (error) {
            console.error(error);
            this.add_toast(new Adw.Toast({title: _('Could not save the access token to the keyring')}));
        }
    }

    async _onClosed() {
        // Preferences take effect without an explicit apply, so keep tokens that
        // were typed in but not applied yet.
        const results = await Promise.allSettled(this._tokens
            .filter(token => token.row.sensitive)
            .map(token => this._saveToken(token)));

        for (const result of results) {
            if (result.status === 'rejected') {
                console.error(result.reason);
            }
        }
        if (this._changed) {
            this.emit('settings-changed');
        }
    }
});
