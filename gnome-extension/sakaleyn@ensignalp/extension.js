// Sakaleyn: ezan vakitleri için GNOME Shell eklentisi.
//
// Eklenti yalnızca gösterir: vakitler, ön uyarı, ezan ve bildirimler ayrı
// kurulan `sakaleyn` servisindedir (D-Bus: io.github.ensignalp.Sakaleyn).
// Eklenti kapalıyken ya da GNOME yeniden başlarken de ezan okunur.
//
// Görünüm yerleri (ayarlardan açılıp kapanır): üst panel göstergesi,
// Hızlı Ayarlar kutucuğu, tarih penceresinde vakitler kartı.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import St from 'gi://St';

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {SakaleynClient} from './lib/client.js';
import {SakaleynDateMenu} from './lib/datemenu.js';
import {ADHAN_VAKITS, Strings, langOf} from './lib/i18n.js';
import {SakaleynPopup, phase, sakaleynGicon, target} from './lib/popup.js';
import {SakaleynQuickSettings} from './lib/quicksettings.js';

const PANEL_KEYS = ['panel-format', 'panel-show-icon', 'panel-show-name', 'panel-highlight'];

const SakaleynIndicator = GObject.registerClass(
class SakaleynIndicator extends PanelMenu.Button {
    _init(ctx) {
        super._init(0.5, 'Sakaleyn', false);
        this._ctx = ctx;
        // Tema `system-status-icon`a geniş dolgu verdiği için kendi sınıfı kullanılır;
        // ad ve süre ayrı etiketler: aralık sabit, süre etiketi sabit genişlikte (kayma yok).
        const box = new St.BoxLayout({style_class: 'sakaleyn-panel', y_align: Clutter.ActorAlign.CENTER});
        this._icon = new St.Icon({gicon: sakaleynGicon(ctx.extPath), style_class: 'sakaleyn-panel-icon',
            y_align: Clutter.ActorAlign.CENTER});
        this._label = new St.Label({style_class: 'sakaleyn-panel-label', y_align: Clutter.ActorAlign.CENTER});
        this._time = new St.Label({style_class: 'sakaleyn-panel-time', y_align: Clutter.ActorAlign.CENTER});
        box.add_child(this._icon);
        box.add_child(this._label);
        box.add_child(this._time);
        this.add_child(box);
        this._box = box;

        this.menu.actor.add_style_class_name('sakaleyn-menu');
        this._popup = new SakaleynPopup(ctx);
        // Etkinleşmeyen ama duyarlı öğe: tıklama menüyü kapatmaz, metin temanın
        // "etkin değil" (soluk) rengini almaz.
        const item = new PopupMenu.PopupBaseMenuItem({
            reactive: true, activate: false, hover: false, can_focus: false, style_class: 'sakaleyn-menu-item',
        });
        // Tema `.popup-menu-item:hover`da arka planı açar; pencere üzerine gelince değişmesin.
        item.track_hover = false;
        item.add_child(this._popup.actor);
        this.menu.addMenuItem(item);
        this.menu.connectObject('open-state-changed', (_m, open) => {
            if (open)
                this._popup.animateIn();
        }, this);
    }

    get popup() {
        return this._popup;
    }

    update(state, error) {
        this._popup.update(state, error);
        this.tick();
    }

    tick() {
        const {settings} = this._ctx;
        const s = this._ctx.strings();
        const state = this._ctx.state.data;
        const fmt = settings.get_string('panel-format');
        this._icon.visible = settings.get_boolean('panel-show-icon') || fmt === 'icon' || !state;
        const t = target(state);
        if (!t || fmt === 'icon') {
            this._label.visible = false;
            this._time.visible = false;
        } else {
            const parts = [];
            if (settings.get_boolean('panel-show-name'))
                parts.push(t.kind === 'iftar' ? (s.lang === 'en' ? 'Iftar' : 'İftar') : s.vakit(t.vakit));
            if (fmt === 'time' || fmt === 'both')
                parts.push(t.local);
            this._label.text = parts.join(' ');
            this._label.visible = parts.length > 0;
            this._time.text = s.countdown(t.ms);
            this._time.visible = fmt === 'remaining' || fmt === 'both';
        }
        const ph = settings.get_boolean('panel-highlight') ? phase(state) : 'normal';
        const quiet = state?.muted_today || state?.enabled === false;
        this._box.style_class = `sakaleyn-panel sakaleyn-phase-${ph}${quiet ? ' sakaleyn-muted-today' : ''}`;
        if (this.menu.isOpen)
            this._popup.tick();
    }
});

export default class SakaleynExtension extends Extension {
    enable() {
        this._settings = this.getSettings();
        const ctx = {
            extPath: this.path,
            settings: this._settings,
            state: {data: null, error: null},
            strings: () => this._strings,
            actions: {
                stop: () => this._call('StopAudio'),
                setEnabled: on => this._call('SetSettings', JSON.stringify({enabled: Boolean(on)})),
                prayed: vakit => this._call('Prayed', vakit),
                toggleAdhan: vakit => this._toggleAdhan(vakit),
                openSettings: () => {
                    this._indicator?.menu.close();
                    Main.panel.closeQuickSettings?.();
                    this.openPreferences();
                },
                openPopup: () => this._indicator?.menu.open(),
            },
        };
        this._ctx = ctx;
        this._strings = new Strings('tr');

        this._syncPlacement();
        this._settings.connectObject('changed', (_s, key) => this._onSettingChanged(key), this);

        this._client = new SakaleynClient({
            onState: state => this._onState(state),
            onError: error => this._onError(error),
        });
        this._client.start();

        // Saniyede bir geri sayım (yalnızca metin değişirse çizilir).
        this._tickId = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, 1, () => {
            this._indicator?.tick();
            this._dateMenu?.tick();
            this._qsTick = (this._qsTick ?? 0) + 1;
            if (this._qsTick % 15 === 0)
                this._qs?.tick();
            return GLib.SOURCE_CONTINUE;
        });
    }

    disable() {
        if (this._tickId) {
            GLib.source_remove(this._tickId);
            this._tickId = 0;
        }
        this._settings?.disconnectObject(this);
        this._client?.destroy();
        this._client = null;
        this._destroyIndicator();
        this._destroyQuickSettings();
        this._destroyDateMenu();
        this._ctx = null;
        this._settings = null;
        this._strings = null;
    }

    async _call(method, ...args) {
        try {
            await this._client?.call(method, ...args);
        } catch (e) {
            Main.notify('Sakaleyn', e.message);
        }
    }

    _toggleAdhan(vakit) {
        const on = this._ctx.state.data?.adhan ?? {};
        const vakits = ADHAN_VAKITS.filter(v => (v === vakit ? !on[v] : on[v]));
        this._call('SetSettings', JSON.stringify({adhan: {vakits}}));
    }

    _onState(state) {
        this._ctx.state = {data: state, error: null};
        if (this._strings.lang !== langOf(state))
            this._strings = new Strings(langOf(state));
        this._render();
    }

    _onError(error) {
        // Servis yeniden başlıyorsa son durum gösterilmeye devam eder.
        if (error.kind === 'service-missing')
            this._ctx.state = {data: null, error};
        else
            this._ctx.state.error = error;
        this._render();
    }

    _render() {
        const {data, error} = this._ctx.state;
        this._indicator?.update(data, error);
        this._qs?.update(data);
        this._dateMenu?.update(data);
    }

    // ------------------------------------------------------------ yerleşim

    _onSettingChanged(key) {
        if (key === 'panel-position' || key === 'panel-index' || key.endsWith('-enabled')) {
            this._syncPlacement();
            this._render();
        } else if (PANEL_KEYS.includes(key)) {
            this._indicator?.tick();
        }
    }

    _syncPlacement() {
        const st = this._settings;
        if (st.get_boolean('panel-enabled'))
            this._createIndicator();
        else
            this._destroyIndicator();
        if (st.get_boolean('qs-enabled'))
            this._createQuickSettings();
        else
            this._destroyQuickSettings();
        if (st.get_boolean('datemenu-enabled'))
            this._createDateMenu();
        else
            this._destroyDateMenu();
    }

    _createIndicator() {
        this._destroyIndicator();
        this._indicator = new SakaleynIndicator(this._ctx);
        Main.panel.addToStatusArea(this.uuid, this._indicator,
            this._settings.get_int('panel-index'), this._settings.get_string('panel-position'));
    }

    _destroyIndicator() {
        this._indicator?.destroy();
        this._indicator = null;
    }

    _createQuickSettings() {
        if (this._qs)
            return;
        const qs = Main.panel.statusArea.quickSettings;
        this._qs = new SakaleynQuickSettings(this._ctx);
        // Hızlı Ayarlar bazı öğelerini eşzamansız kurar; hazır olana kadar bekle.
        const attach = () => {
            if (!this._qs)
                return GLib.SOURCE_REMOVE;
            if (!qs._indicators)
                return GLib.SOURCE_CONTINUE;
            this._qs.attach(qs);
            this._qs.update(this._ctx.state.data);
            this._qsAttachId = 0;
            return GLib.SOURCE_REMOVE;
        };
        if (attach() === GLib.SOURCE_CONTINUE)
            this._qsAttachId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 200, attach);
    }

    _destroyQuickSettings() {
        if (this._qsAttachId) {
            GLib.source_remove(this._qsAttachId);
            this._qsAttachId = 0;
        }
        this._qs?.destroy();
        this._qs = null;
    }

    _createDateMenu() {
        if (this._dateMenu || !Main.panel.statusArea.dateMenu)
            return;
        this._dateMenu = new SakaleynDateMenu(this._ctx);
        this._dateMenu.update(this._ctx.state.data);
    }

    _destroyDateMenu() {
        this._dateMenu?.destroy();
        this._dateMenu = null;
    }
}
