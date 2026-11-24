// Sakaleyn: ayarlar arayüzü (GTK 4 + libadwaita).
//
// İki yerde kullanılır: GNOME eklenti tercihleri (prefs.js) ve diğer
// masaüstleri için bağımsız pencere (`sakaleyn settings` → settings/main.js).
// Vakit, uyarı ve ses ayarları servise D-Bus ile yazılır (config.toml);
// "Görünüm" sayfası yalnızca eklentide vardır (GSettings).

import Adw from 'gi://Adw';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Gtk from 'gi://Gtk';

import {call, describe, newProxy} from './lib/client.js';
import {ADHAN_VAKITS, Strings, VAKITS, langOf} from './lib/i18n.js';

function isObject(v) {
    return v && typeof v === 'object' && !Array.isArray(v);
}

function merge(base, patch) {
    for (const [k, v] of Object.entries(patch)) {
        if (isObject(v) && isObject(base[k]))
            merge(base[k], v);
        else
            base[k] = v;
    }
}

class Ui {
    constructor(proxy, cfg, state, sounds, strings) {
        this.proxy = proxy;
        this.cfg = cfg;
        this.state = state;
        this.sounds = sounds;
        this.s = strings;
    }

    patch(obj) {
        merge(this.cfg, obj);
        call(this.proxy, 'SetSettings', JSON.stringify(obj)).catch(e => logError(e, 'Sakaleyn SetSettings'));
    }

    group(title = '', description = '') {
        return new Adw.PreferencesGroup({title, description});
    }

    switchRow(title, subtitle, get, set) {
        const row = new Adw.SwitchRow({title, subtitle: subtitle ?? '', active: Boolean(get())});
        row.connect('notify::active', () => set(row.active));
        return row;
    }

    spinRow(title, subtitle, min, max, get, set) {
        const row = Adw.SpinRow.new_with_range(min, max, 1);
        row.title = title;
        row.subtitle = subtitle ?? '';
        row.value = get();
        row.connect('notify::value', () => set(Math.round(row.value)));
        return row;
    }

    comboRow(title, items, getId, setId, subtitle = '') {
        const row = new Adw.ComboRow({title, subtitle, model: Gtk.StringList.new(items.map(i => i.title))});
        const idx = items.findIndex(i => i.id === getId());
        row.selected = idx >= 0 ? idx : 0;
        row.connect('notify::selected', () => setId(items[row.selected]?.id));
        return row;
    }

    /** Ses seçimi + "Dinle" düğmesi. `kinds`: listelenecek ses türleri. */
    soundRow(title, kinds, getId, setId, extra = []) {
        const items = [...extra, ...this.sounds.filter(x => kinds.includes(x.kind)).map(x => ({id: x.id, title: x.title}))];
        const row = this.comboRow(title, items, getId, setId);
        const play = new Gtk.Button({
            icon_name: 'media-playback-start-symbolic',
            tooltip_text: this.s.t('preview'),
            valign: Gtk.Align.CENTER,
            css_classes: ['flat', 'circular'],
        });
        play.connect('clicked', () => {
            const id = items[row.selected]?.id;
            if (id && id !== 'none')
                call(this.proxy, 'PreviewSound', id).catch(e => logError(e));
        });
        row.add_suffix(play);
        return row;
    }

    volumeRow(title, get, set) {
        const row = new Adw.ActionRow({title});
        const scale = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, 0, 100, 5);
        scale.set_size_request(180, -1);
        scale.valign = Gtk.Align.CENTER;
        scale.set_value(Math.round(get() * 100));
        scale.connect('value-changed', () => set(scale.get_value() / 100));
        row.add_suffix(scale);
        return row;
    }

    buttonRow(title, label, onClick, subtitle = '') {
        const row = new Adw.ActionRow({title, subtitle});
        const b = new Gtk.Button({label, valign: Gtk.Align.CENTER});
        b.connect('clicked', onClick);
        row.add_suffix(b);
        row.activatable_widget = b;
        return row;
    }

    // ------------------------------------------------------------ sayfalar

    locationPage() {
        const s = this.s;
        const page = new Adw.PreferencesPage({title: s.t('page_location'), icon_name: 'find-location-symbolic'});
        const g = this.group(s.t('page_location'));
        const current = new Adw.ActionRow({title: s.t('current_district')});
        const refresh = new Gtk.Button({
            icon_name: 'view-refresh-symbolic', tooltip_text: s.t('refresh_times'),
            valign: Gtk.Align.CENTER, css_classes: ['flat'],
        });
        refresh.connect('clicked', () => call(this.proxy, 'RefreshTimes').catch(e => logError(e)));
        current.add_suffix(refresh);
        this._currentRow = current;
        const auto = this.switchRow(s.t('auto_location'), s.t('auto_location_sub'),
            () => this.cfg.location?.mode !== 'manual',
            on => {
                if (on)
                    call(this.proxy, 'UseAutoLocation').catch(e => logError(e));
                else
                    this.patch({location: {mode: 'manual'}});
            });
        this._autoRow = auto;
        g.add(auto);
        g.add(current);
        page.add(g);

        const sg = this.group('', s.t('search_hint'));
        const entry = new Gtk.SearchEntry({placeholder_text: s.t('search_district'), hexpand: true});
        const list = new Gtk.ListBox({css_classes: ['boxed-list'], selection_mode: Gtk.SelectionMode.NONE});
        list.visible = false;
        let gen = 0;
        entry.connect('search-changed', async () => {
            const q = entry.text.trim();
            const my = ++gen;
            let child;
            while ((child = list.get_first_child()))
                list.remove(child);
            if (q.length < 2) {
                list.visible = false;
                return;
            }
            const res = JSON.parse(await call(this.proxy, 'SearchDistricts', q));
            if (my !== gen)
                return;
            for (const d of res.slice(0, 15)) {
                // semt/ilçe Diyanet listesinde yoksa hangi tabloyu kullandığı gösterilir
                const row = new Adw.ActionRow({title: d.name, subtitle: d.via ? s.timesOf(d.via) : (d.tz ?? ''),
                    activatable: true});
                row.connect('activated', async () => {
                    await call(this.proxy, 'SetDistrict', d.id).catch(e => logError(e));
                    this.cfg.location = {mode: 'manual', district_id: d.id};
                    this._autoRow.active = false;
                    entry.text = '';
                });
                list.append(row);
            }
            list.visible = res.length > 0;
        });
        const box = new Gtk.Box({orientation: Gtk.Orientation.VERTICAL, spacing: 12});
        box.append(entry);
        box.append(list);
        sg.add(box);
        page.add(sg);
        this.updateLocation(this.state);
        return page;
    }

    updateLocation(state) {
        this.state = state;
        if (!this._currentRow)
            return;
        const s = this.s;
        this._currentRow.title = state?.district?.name ?? '—';
        const bits = [];
        if (state?.location?.status === 'pending')
            bits.push(s.t('location_pending'));
        if (state?.location?.guessed)
            bits.push(s.t('location_guess'));
        if (state?.today?.source === 'calculated')
            bits.push(s.t('estimated'));
        if (state?.fetched_at)
            bits.push(`${s.t('last_update')}: ${GLib.DateTime.new_from_iso8601(state.fetched_at, null)?.to_local().format('%d.%m.%Y %H:%M') ?? ''}`);
        this._currentRow.subtitle = bits.join(' · ');
    }

    alertsPage() {
        const s = this.s;
        const a = () => this.cfg.alerts;
        const page = new Adw.PreferencesPage({title: s.t('page_alerts'), icon_name: 'alarm-symbolic'});
        const g = this.group(s.t('page_alerts'));
        g.add(this.switchRow(s.t('alerts_enabled'), '', () => a().enabled, v => this.patch({alerts: {enabled: v}})));
        g.add(this.spinRow(s.t('alerts_minutes'), s.t('alerts_minutes_sub'), 1, 180,
            () => a().minutes, v => this.patch({alerts: {minutes: v}})));
        g.add(this.soundRow(s.t('sound'), ['alert', 'user'], () => a().sound, id => this.patch({alerts: {sound: id}}),
            [{id: 'none', title: s.t('no_sound')}]));
        g.add(this.volumeRow(s.t('volume'), () => a().volume, v => this.patch({alerts: {volume: v}})));
        g.add(this.switchRow(s.t('play_in_dnd'), '', () => a().play_in_dnd, v => this.patch({alerts: {play_in_dnd: v}})));
        g.add(this.spinRow(s.t('snooze'), '', 1, 60, () => a().snooze_minutes, v => this.patch({alerts: {snooze_minutes: v}})));
        g.add(this.buttonRow(s.t('test_alert'), s.t('test'),
            () => call(this.proxy, 'TestEvent', 'pre_alert', 'ogle').catch(e => logError(e))));
        page.add(g);

        const pv = this.group(s.t('per_vakit'), s.t('per_vakit_sub'));
        for (const v of ADHAN_VAKITS) {
            // Geçerli süre gösterilir; genel süreye eşitlenirse vakite özel ayar silinir.
            const row = this.spinRow(s.vakit(v), '', 1, 180, () => a().per_vakit?.[v] ?? a().minutes, m => {
                const per = {...(a().per_vakit ?? {})};
                if (m !== a().minutes)
                    per[v] = m;
                else
                    delete per[v];
                // nesne birleşmesin diye tamamı gönderilir
                this.cfg.alerts.per_vakit = per;
                call(this.proxy, 'SetSettings', JSON.stringify({alerts: {per_vakit: null}}))
                    .then(() => call(this.proxy, 'SetSettings', JSON.stringify({alerts: {per_vakit: per}})))
                    .catch(e => logError(e));
            });
            const sw = new Gtk.Switch({active: a().vakits.includes(v), valign: Gtk.Align.CENTER});
            sw.connect('notify::active', () => {
                const set = new Set(a().vakits);
                if (sw.active)
                    set.add(v);
                else
                    set.delete(v);
                this.patch({alerts: {vakits: VAKITS.filter(x => set.has(x))}});
            });
            row.add_prefix(sw);
            pv.add(row);
        }
        page.add(pv);
        return page;
    }

    adhanPage() {
        const s = this.s;
        const h = () => this.cfg.adhan;
        const page = new Adw.PreferencesPage({title: s.t('page_adhan'), icon_name: 'audio-volume-high-symbolic'});
        const g = this.group(s.t('page_adhan'));
        g.add(this.switchRow(s.t('adhan_enabled'), '', () => h().enabled, v => this.patch({adhan: {enabled: v}})));
        g.add(this.volumeRow(s.t('volume'), () => h().volume, v => this.patch({adhan: {volume: v}})));
        g.add(this.switchRow(s.t('pause_media'), s.t('pause_media_sub'), () => h().pause_media,
            v => this.patch({adhan: {pause_media: v}})));
        g.add(this.switchRow(s.t('play_in_dnd'), '', () => h().play_in_dnd, v => this.patch({adhan: {play_in_dnd: v}})));
        const test = new Adw.ActionRow({title: s.t('test_adhan')});
        const tb = new Gtk.Button({label: s.t('test'), valign: Gtk.Align.CENTER});
        tb.connect('clicked', () => call(this.proxy, 'TestEvent', 'adhan', 'ogle').catch(e => logError(e)));
        const sb = new Gtk.Button({label: s.t('stop'), valign: Gtk.Align.CENTER, css_classes: ['destructive-action']});
        sb.connect('clicked', () => call(this.proxy, 'StopAudio').catch(e => logError(e)));
        test.add_suffix(tb);
        test.add_suffix(sb);
        g.add(test);
        page.add(g);

        const pv = this.group(s.t('per_vakit'), s.t('adhan_vakit_sub'));
        for (const v of ADHAN_VAKITS) {
            const row = this.soundRow(s.vakit(v), ['adhan', 'user'], () => h().sounds?.[v],
                id => this.patch({adhan: {sounds: {[v]: id}}}));
            const sw = new Gtk.Switch({active: h().vakits.includes(v), valign: Gtk.Align.CENTER});
            sw.connect('notify::active', () => {
                const set = new Set(h().vakits);
                if (sw.active)
                    set.add(v);
                else
                    set.delete(v);
                this.patch({adhan: {vakits: VAKITS.filter(x => set.has(x))}});
            });
            row.add_prefix(sw);
            pv.add(row);
        }
        pv.add(this.soundRow(s.t('friday_sound'), ['adhan', 'user'], () => this.cfg.friday.sound,
            id => this.patch({friday: {sound: id}}), [{id: '', title: s.t('same_as_ogle')}]));
        page.add(pv);

        const ug = this.group();
        ug.add(this.buttonRow(s.t('add_sounds'), s.t('open_folder'), () => {
            const dir = GLib.build_filenamev([GLib.get_user_data_dir(), 'sakaleyn', 'sounds']);
            GLib.mkdir_with_parents(dir, 0o755);
            Gio.AppInfo.launch_default_for_uri(GLib.filename_to_uri(dir, null), null);
        }, s.t('add_sounds_sub')));
        page.add(ug);
        return page;
    }

    morePage() {
        const s = this.s;
        const c = this.cfg;
        const page = new Adw.PreferencesPage({title: s.t('page_more'), icon_name: 'view-more-symbolic'});

        const fr = this.group(s.t('friday_title'));
        fr.add(this.switchRow(s.t('friday_enabled'), '', () => c.friday.enabled, v => this.patch({friday: {enabled: v}})));
        fr.add(this.spinRow(s.t('friday_minutes'), '', 5, 240, () => c.friday.minutes, v => this.patch({friday: {minutes: v}})));
        page.add(fr);

        const rm = this.group(s.t('ramadan_title'));
        rm.add(this.switchRow(s.t('ramadan_enabled'), s.t('ramadan_enabled_sub'), () => c.ramadan.enabled,
            v => this.patch({ramadan: {enabled: v}})));
        rm.add(this.spinRow(s.t('sahur_minutes'), '', 0, 180, () => c.ramadan.sahur_minutes,
            v => this.patch({ramadan: {sahur_minutes: v}})));
        rm.add(this.soundRow(s.t('sahur_sound'), ['alert', 'user'], () => c.ramadan.sahur_sound,
            id => this.patch({ramadan: {sahur_sound: id}}), [{id: 'none', title: s.t('no_sound')}]));
        page.add(rm);

        const ex = this.group(s.t('extra_title'));
        ex.add(this.switchRow(s.t('sunrise_alert'), s.t('sunrise_alert_sub'), () => c.sunrise.alert,
            v => this.patch({sunrise: {alert: v}})));
        ex.add(this.spinRow(`${s.t('sunrise_alert')} · ${s.t('alerts_minutes')}`, '', 1, 120, () => c.sunrise.minutes,
            v => this.patch({sunrise: {minutes: v}})));
        ex.add(this.switchRow(s.t('end_reminder'), s.t('end_reminder_sub'), () => c.end_reminder.enabled,
            v => this.patch({end_reminder: {enabled: v}})));
        ex.add(this.spinRow(`${s.t('end_reminder')} · ${s.t('alerts_minutes')}`, '', 1, 120, () => c.end_reminder.minutes,
            v => this.patch({end_reminder: {minutes: v}})));
        ex.add(this.switchRow(s.t('kerahat'), s.t('kerahat_sub'), () => c.kerahat.enabled,
            v => this.patch({kerahat: {enabled: v}})));
        ex.add(this.switchRow(s.t('religious_days'), '', () => c.religious_days.enabled,
            v => this.patch({religious_days: {enabled: v}})));
        page.add(ex);

        const of = this.group(s.t('offsets'), s.t('offsets_sub'));
        for (const v of VAKITS) {
            of.add(this.spinRow(s.vakit(v), '', -60, 60, () => c.offsets?.[v] ?? 0, m => {
                const all = {...(c.offsets ?? {})};
                all[v] = m;
                this.patch({offsets: all});
            }));
        }
        page.add(of);

        const lg = this.group(s.t('language'));
        lg.add(this.comboRow(s.t('language'), [
            {id: 'tr', title: s.t('lang_tr')}, {id: 'en', title: s.t('lang_en')}, {id: 'auto', title: s.t('lang_auto')},
        ], () => c.language, id => this.patch({language: id})));
        page.add(lg);

        const ab = this.group(s.t('about'), s.t('data_sources'));
        ab.add(new Adw.ActionRow({title: s.t('service_version'), subtitle: `${this.state?.version ?? '?'} · ${this.state?.player ?? '-'}`}));
        page.add(ab);
        return page;
    }

    appearancePage(settings) {
        const s = this.s;
        const page = new Adw.PreferencesPage({title: s.t('page_appearance'), icon_name: 'preferences-desktop-appearance-symbolic'});
        const g = this.group(s.t('panel_title'));
        const bindSwitch = (key, title) => {
            const row = new Adw.SwitchRow({title});
            settings.bind(key, row, 'active', Gio.SettingsBindFlags.DEFAULT);
            g.add(row);
        };
        const combo = (key, title, items) => {
            g.add(this.comboRow(title, items, () => settings.get_string(key), id => settings.set_string(key, id)));
        };
        bindSwitch('panel-enabled', s.t('panel_enabled'));
        combo('panel-position', s.t('panel_position'), [
            {id: 'left', title: s.t('pos_left')}, {id: 'center', title: s.t('pos_center')}, {id: 'right', title: s.t('pos_right')},
        ]);
        const idx = Adw.SpinRow.new_with_range(-1, 20, 1);
        idx.title = s.t('panel_index');
        settings.bind('panel-index', idx, 'value', Gio.SettingsBindFlags.DEFAULT);
        g.add(idx);
        combo('panel-format', s.t('panel_format'), [
            {id: 'remaining', title: s.t('fmt_remaining')}, {id: 'time', title: s.t('fmt_time')},
            {id: 'both', title: s.t('fmt_both')}, {id: 'icon', title: s.t('fmt_icon')},
        ]);
        bindSwitch('panel-show-icon', s.t('panel_show_icon'));
        bindSwitch('panel-show-name', s.t('panel_show_name'));
        bindSwitch('panel-highlight', s.t('panel_highlight'));
        bindSwitch('qs-enabled', s.t('qs_enabled'));
        bindSwitch('datemenu-enabled', s.t('datemenu_enabled'));
        page.add(g);
        return page;
    }
}

function errorPage(error, s) {
    const page = new Adw.PreferencesPage({title: 'Sakaleyn', icon_name: 'dialog-warning-symbolic'});
    const g = new Adw.PreferencesGroup();
    const status = new Adw.StatusPage({
        icon_name: 'dialog-warning-symbolic',
        title: s.t('service_missing'),
        description: error.kind === 'service-missing' ? s.t('service_missing_body') : error.message,
    });
    g.add(status);
    page.add(g);
    return page;
}

/**
 * Pencereye sayfaları ekler.
 * @param {Adw.PreferencesWindow} window
 * @param {Gio.Settings|null} settings eklenti GSettings'i (bağımsız pencerede null)
 */
export async function fillWindow(window, settings = null) {
    window.set_default_size(640, 760);
    window.search_enabled = true;
    let proxy, state, cfg, sounds;
    try {
        proxy = await newProxy();
        state = JSON.parse(await call(proxy, 'GetState'));
        cfg = JSON.parse(await call(proxy, 'GetSettings'));
        sounds = JSON.parse(await call(proxy, 'ListSounds'));
    } catch (e) {
        const s = new Strings('tr');
        window.add(errorPage(describe(e), s));
        if (settings)
            window.add(new Ui(null, {}, null, [], s).appearancePage(settings));
        return;
    }
    const ui = new Ui(proxy, cfg, state, sounds, new Strings(langOf(state)));
    window.add(ui.locationPage());
    window.add(ui.alertsPage());
    window.add(ui.adhanPage());
    window.add(ui.morePage());
    if (settings)
        window.add(ui.appearancePage(settings));

    // Konum/ilçe değişikliklerini canlı göster.
    const id = proxy.connectSignal('StateChanged', (_p, _s, [json]) => {
        try {
            ui.updateLocation(JSON.parse(json));
        } catch (e) {
            logError(e);
        }
    });
    window.connect('close-request', () => {
        proxy.disconnectSignal(id);
        // pencere kapanınca deneme sesi susmasın diye bilerek durdurulmaz
        return false;
    });
}
