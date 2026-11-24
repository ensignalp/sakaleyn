// Sakaleyn: açılır pencere. Üstte logo (ayarları açar), ilçe ve tarih; ortada
// şimdiki vaktin ilerlemesini gösteren yarım daire ve içinde geri sayım;
// altında altı vakit (ezan zili, "kıldım"), en altta uyarılar.
//
// Yarım daire metne göre boyutlanır: yarıçap, metin bloğunun her satırının üst
// köşeleri yayın iç kenarına değmeyecek şekilde hesaplanır (yazı ölçeği büyüse de
// çakışmaz), en fazla pencere genişliği kadar büyür.

import Cairo from 'cairo';
import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import Pango from 'gi://Pango';
import St from 'gi://St';

import {ADHAN_VAKITS, VAKITS} from './i18n.js';

const VERTICAL = Clutter.Orientation.VERTICAL;
// yarım daire göstergesi (px)
const GAUGE_LINE = 12; // yay kalınlığı
const GAUGE_GAP = 10; // metin ile yayın iç kenarı arası
const GAUGE_MIN_R = 92; // yay ekseninin en küçük yarıçapı
const GAUGE_MAX_W = 300; // pencere içine sığan en büyük genişlik
const GAUGE_ANIM_MS = 700;
const TEXT_LIFT = 2; // metnin alt kenarı ile yay ekseni arası
// logo yeşili (#176A52) → vurgu yeşili (#2a9d78), stylesheet.css ile aynı
const GREEN_DARK = [23 / 255, 106 / 255, 82 / 255];
const ACCENT = [42 / 255, 157 / 255, 120 / 255];
const CREAM = [236 / 255, 238 / 255, 204 / 255];

export function sakaleynGicon(extPath, symbolic = true) {
    return Gio.icon_new_for_string(`${extPath}/icons/sakaleyn${symbolic ? '-symbolic' : ''}.svg`);
}

function label(text, styleClass = '', props = {}) {
    return new St.Label({text: String(text ?? ''), style_class: styleClass, y_align: Clutter.ActorAlign.CENTER, ...props});
}

// Simge saydamlıkları (St CSS `opacity` desteklemez; aktörün opacity değeri kullanılır)
const DIM_PASSIVE = 60; // güneş: yer tutan, tıklanamaz simgeler
const DIM_IDLE = 100; // kılınmadı işareti, kapalı ezan zili

/** Düğmeyi yerini koruyarak gizler/gösterir (satırdaki hizayı bozmaz). */
function setSlotVisible(button, on, opacity = 255) {
    button.opacity = on ? opacity : 0;
    button.reactive = on;
    button.can_focus = on;
}

function iconButton(iconName, styleClass, onClick, accessible = '') {
    const b = new St.Button({
        style_class: `sakaleyn-icon-button ${styleClass}`,
        can_focus: true,
        accessible_name: accessible,
        child: new St.Icon({icon_name: iconName, icon_size: 16}),
        y_align: Clutter.ActorAlign.CENTER,
    });
    b.connect('clicked', onClick);
    return b;
}

/** Sıradaki hedef: Ramazan'da iftar öncesi iftar, değilse sıradaki vakit. */
export function target(state, now = Date.now()) {
    if (!state?.next)
        return null;
    if (state.ramadan && state.iftar) {
        const at = Date.parse(state.iftar);
        if (at > now)
            return {kind: 'iftar', vakit: 'aksam', at, local: state.today?.times?.aksam ?? '', ms: at - now};
    }
    const at = Date.parse(state.next.at);
    return {kind: 'vakit', vakit: state.next.vakit, at, local: state.next.local, ms: at - now};
}

/** Panel vurgusu: normal | soon (ön uyarı süresi içinde) | imminent (son 10 dk) | adhan (çalıyor). */
export function phase(state, now = Date.now()) {
    if (state?.playing?.kind === 'adhan')
        return 'adhan';
    const n = state?.next ? {vakit: state.next.vakit, ms: Date.parse(state.next.at) - now} : null;
    if (!n)
        return 'normal';
    if (n.ms <= 10 * 60000)
        return 'imminent';
    const alert = state.alert_minutes?.[n.vakit] ?? 45;
    return n.ms <= alert * 60000 ? 'soon' : 'normal';
}

export class SakaleynPopup {
    /** @param {object} ctx {extPath, strings(), actions} */
    constructor(ctx) {
        this._ctx = ctx;
        this._state = null;
        this.actor = new St.BoxLayout({orientation: VERTICAL, style_class: 'sakaleyn-popup', x_expand: true});

        // başlık: logo ayarları açar
        const head = new St.BoxLayout({style_class: 'sakaleyn-head', x_expand: true});
        this._logo = new St.Button({
            style_class: 'sakaleyn-logo-button',
            can_focus: true,
            child: new St.Icon({gicon: sakaleynGicon(ctx.extPath, false), icon_size: 36, style_class: 'sakaleyn-logo'}),
            y_align: Clutter.ActorAlign.CENTER,
        });
        this._logo.connect('clicked', () => ctx.actions.openSettings());
        head.add_child(this._logo);
        const titles = new St.BoxLayout({orientation: VERTICAL, x_expand: true, y_align: Clutter.ActorAlign.CENTER});
        const place = new St.BoxLayout({style_class: 'sakaleyn-place'});
        place.add_child(new St.Icon({icon_name: 'mark-location-symbolic', icon_size: 14, style_class: 'sakaleyn-place-icon',
            y_align: Clutter.ActorAlign.CENTER}));
        this._district = label('Sakaleyn', 'sakaleyn-district');
        place.add_child(this._district);
        titles.add_child(place);
        // miladi ve hicri tarih aynı satırda; dar alanda hicri tarih kısalır
        const dates = new St.BoxLayout({style_class: 'sakaleyn-dates', x_expand: true});
        this._date = label('', 'sakaleyn-date');
        this._date.clutter_text.ellipsize = Pango.EllipsizeMode.NONE;
        this._hijri = label('', 'sakaleyn-date sakaleyn-muted', {x_expand: true});
        this._hijri.clutter_text.ellipsize = Pango.EllipsizeMode.END;
        dates.add_child(this._date);
        dates.add_child(this._hijri);
        titles.add_child(dates);
        head.add_child(titles);
        this.actor.add_child(head);

        // geri sayım: yarım daire göstergesi, metin dairenin iç boşluğunda ortalı
        // Metin bloğu yayın tabanına açıkça yerleştirilir (_placeTexts): hizalamaya
        // güvenilmez, yarıçap hesabı ile yerleşim aynı geometriyi kullanır.
        const cd = new St.Widget({
            layout_manager: new Clutter.FixedLayout(),
            style_class: 'sakaleyn-countdown',
            x_align: Clutter.ActorAlign.CENTER,
        });
        this._frac = 0; // hedef ilerleme
        this._shown = 0; // çizilen ilerleme (animasyonlu)
        this._radius = GAUGE_MIN_R;
        this._gauge = new St.DrawingArea({style_class: 'sakaleyn-gauge'});
        this._gauge.connect('repaint', area => this._drawGauge(area));
        cd.add_child(this._gauge);
        const texts = new St.BoxLayout({orientation: VERTICAL, style_class: 'sakaleyn-gauge-texts'});
        this._texts = texts;
        this._caption = label('', 'sakaleyn-caption', {x_align: Clutter.ActorAlign.CENTER});
        this._big = label('', 'sakaleyn-big', {x_align: Clutter.ActorAlign.CENTER});
        this._sub = label('', 'sakaleyn-sub sakaleyn-muted', {x_align: Clutter.ActorAlign.CENTER});
        texts.add_child(this._caption);
        texts.add_child(this._big);
        texts.add_child(this._sub);
        cd.add_child(texts);
        this.actor.add_child(cd);
        this._setRadius(GAUGE_MIN_R);

        // vakitler
        this._list = new St.BoxLayout({orientation: VERTICAL, style_class: 'sakaleyn-list', x_expand: true});
        this._rows = {};
        for (const v of VAKITS) {
            const row = new St.BoxLayout({style_class: 'sakaleyn-row', x_expand: true});
            const name = label('', 'sakaleyn-row-name', {x_expand: true});
            const time = label('--:--', 'sakaleyn-row-time');
            row.add_child(name);
            row.add_child(time);
            // Her satırda aynı iki düğme yuvası; saatler hizalı kalır. Güneşte (namaz ve ezan
            // yok) iki simge görünür ama pasiftir; "kıldım" henüz geçmemiş vakitte saydamlaşır.
            const isAdhan = ADHAN_VAKITS.includes(v);
            const prayed = iconButton('object-select-symbolic', 'sakaleyn-prayed', () => ctx.actions.prayed(v));
            const bell = iconButton('audio-volume-high-symbolic', 'sakaleyn-bell', () => ctx.actions.toggleAdhan(v));
            if (!isAdhan) {
                for (const b of [prayed, bell]) {
                    b.reactive = false;
                    b.can_focus = false;
                    b.opacity = DIM_PASSIVE;
                }
            }
            row.add_child(prayed);
            row.add_child(bell);
            this._rows[v] = {row, name, time, bell: isAdhan ? bell : null, prayed: isAdhan ? prayed : null,
                passive: isAdhan ? [] : [prayed, bell]};
            this._list.add_child(row);
        }
        this.actor.add_child(this._list);

        // uyarı; konum bulunamadıysa tıklanınca ayarların Konum sayfası (elle arama) açılır
        this._notice = label('', 'sakaleyn-notice', {x_expand: true});
        this._notice.clutter_text.line_wrap = true;
        this._noticeButton = new St.Button({style_class: 'sakaleyn-notice-button', child: this._notice, x_expand: true,
            reactive: false, can_focus: false});
        this._noticeButton.connect('clicked', () => ctx.actions.openSettings());
        this.actor.add_child(this._noticeButton);
        this._religious = label('', 'sakaleyn-religious sakaleyn-muted', {x_expand: true, x_align: Clutter.ActorAlign.CENTER});
        this.actor.add_child(this._religious);

        // yalnızca ses çalarken görünen "Sustur"
        const actions = new St.BoxLayout({style_class: 'sakaleyn-actions', x_expand: true});
        this._stop = new St.Button({style_class: 'button sakaleyn-button sakaleyn-stop', can_focus: true, x_expand: true});
        this._stop.connect('clicked', () => ctx.actions.stop());
        actions.add_child(this._stop);
        this.actor.add_child(actions);
    }

    /**
     * Yarıçapı metne göre ayarlar. Her satırın üst köşesi (yarım genişlik, tabandan
     * yükseklik) yayın iç kenarının en az GAUGE_GAP içinde kalmalı.
     */
    _fitGauge() {
        if (!this.actor.get_stage())
            return; // ölçüm için tema gerekir; sahneye girince yapılır
        const rows = [this._sub, this._big, this._caption];
        let y = TEXT_LIFT; // metin bloğunun alt kenarı, yay ekseninin bu kadar üstünde
        let need = 0;
        for (const l of rows) {
            if (!l.visible || !l.text)
                continue;
            const [, w] = l.get_preferred_width(-1);
            const [, h] = l.get_preferred_height(w);
            y += h;
            need = Math.max(need, Math.hypot(w / 2, y));
        }
        const maxR = (GAUGE_MAX_W - GAUGE_LINE) / 2 - 1;
        this._setRadius(Math.min(maxR, Math.max(GAUGE_MIN_R, Math.ceil(need + GAUGE_GAP + GAUGE_LINE / 2))));
        this._placeTexts();
    }

    /** Metin bloğunu yay merkezine yatayda ortalar, alt kenarını eksenin TEXT_LIFT üstüne koyar. */
    _placeTexts() {
        const [, w] = this._texts.get_preferred_width(-1);
        const [, h] = this._texts.get_preferred_height(w);
        const cx = this._gauge.width / 2;
        const axis = this._gauge.height - GAUGE_LINE / 2 - 1;
        this._texts.set_position(Math.round(cx - w / 2), Math.round(axis - TEXT_LIFT - h));
    }

    _setRadius(r) {
        if (r === this._radius && this._gauge.width > 0)
            return;
        this._radius = r;
        // yay ekseni ve yuvarlak uçlar sığsın: genişlik 2r + kalınlık, yükseklik r + kalınlık
        this._gauge.set_size(2 * r + GAUGE_LINE + 2, r + GAUGE_LINE + 1);
        this._gauge.queue_repaint();
    }

    /** Yarım daire: soldan sağa iz, üstünde geçen kısım (yeşil gradyan) ve uçta düğme. */
    _drawGauge(area) {
        const cr = area.get_context();
        const [w, h] = area.get_surface_size();
        const r = this._radius;
        const cx = w / 2;
        const cy = h - GAUGE_LINE / 2 - 1;
        const fg = area.get_theme_node().get_foreground_color();
        cr.setLineWidth(GAUGE_LINE);
        cr.setLineCap(Cairo.LineCap.ROUND);
        cr.setSourceRGBA(fg.red / 255, fg.green / 255, fg.blue / 255, 0.13);
        cr.arc(cx, cy, r, Math.PI, 2 * Math.PI);
        cr.stroke();

        const frac = Math.min(1, Math.max(0, this._shown));
        const end = Math.PI + Math.PI * frac;
        if (frac > 0.002) {
            const grad = new Cairo.LinearGradient(cx - r, 0, cx + r, 0);
            grad.addColorStopRGBA(0, ...GREEN_DARK, 1);
            grad.addColorStopRGBA(1, ...ACCENT, 1);
            cr.setSource(grad);
            cr.arc(cx, cy, r, Math.PI, end);
            cr.stroke();
        }
        // uçtaki düğme: krem dolgu, yeşil halka (başlangıçta da görünür)
        const kx = cx + r * Math.cos(end);
        const ky = cy + r * Math.sin(end);
        cr.setSourceRGBA(...ACCENT, 1);
        cr.arc(kx, ky, GAUGE_LINE * 0.72, 0, 2 * Math.PI);
        cr.fill();
        cr.setSourceRGBA(...CREAM, 1);
        cr.arc(kx, ky, GAUGE_LINE * 0.38, 0, 2 * Math.PI);
        cr.fill();
        cr.$dispose();
    }

    /** Çizilen ilerlemeyi hedefe yumuşakça götürür (ease-out). */
    _animateTo(target, from = this._shown) {
        this._timeline?.stop();
        const start = from;
        this._timeline = new Clutter.Timeline({actor: this._gauge, duration: GAUGE_ANIM_MS});
        this._timeline.connect('new-frame', tl => {
            const p = tl.get_progress();
            const eased = 1 - (1 - p) ** 3;
            this._shown = start + (target - start) * eased;
            this._gauge.queue_repaint();
        });
        this._timeline.connect('completed', () => {
            this._shown = target;
            this._timeline = null;
            this._gauge.queue_repaint();
        });
        this._timeline.start();
    }

    /** Pencere açılınca yay baştan dolar. */
    animateIn() {
        this.tick();
        this._animateTo(this._frac, 0);
    }

    update(state, error) {
        this._state = state;
        const s = this._ctx.strings();
        this._stop.label = s.t('stop');
        this._logo.accessible_name = s.t('settings');
        if (!state) {
            this._district.text = 'Sakaleyn';
            this._date.text = error?.kind === 'service-missing' ? s.t('service_missing') : s.t('waiting');
            this._hijri.text = '';
            this._caption.text = '';
            this._big.text = '';
            this._sub.text = '';
            this._notice.text = error?.kind === 'service-missing' ? s.t('service_missing_body') : (error?.message ?? '');
            this._noticeButton.visible = Boolean(this._notice.text);
            this._noticeButton.reactive = false;
            this._list.visible = false;
            this._religious.visible = false;
            this._stop.visible = false;
            return;
        }
        this._list.visible = true;
        this._district.text = state.district?.name ?? 'Sakaleyn';
        this._date.text = state.today ? s.longDate(state.today.date, state.today.weekday) : '';
        this._hijri.text = state.today?.hijri ? `· ${state.today.hijri}` : '';
        this._hijri.visible = Boolean(this._hijri.text);

        const current = state.current?.vakit;
        // Sıradaki vakit bugünün listesindeyse işaretlenir (yatsıdan sonra sıradaki yarının imsakıdır).
        const nextToday = state.next && state.today?.instants?.[state.next.vakit] &&
            Date.parse(state.today.instants[state.next.vakit]) === Date.parse(state.next.at);
        const next = nextToday ? state.next.vakit : null;
        for (const v of VAKITS) {
            const r = this._rows[v];
            r.name.text = s.vakit(v);
            r.time.text = state.today?.times?.[v] ?? '--:--';
            const passed = state.today?.instants?.[v] && Date.parse(state.today.instants[v]) <= Date.now();
            r.row.style_class = `sakaleyn-row${v === current ? ' sakaleyn-current' : ''}${v === next ? ' sakaleyn-next' : ''}${passed && v !== current ? ' sakaleyn-past' : ''}`;
            if (r.bell) {
                const on = Boolean(state.adhan?.[v]);
                r.bell.child.icon_name = on ? 'audio-volume-high-symbolic' : 'audio-volume-muted-symbolic';
                r.bell.accessible_name = on ? s.t('adhan_on') : s.t('adhan_off');
                r.bell.opacity = on ? 255 : DIM_IDLE;
            }
            for (const b of r.passive)
                b.accessible_name = s.t('sunrise_no_adhan');
            if (r.prayed) {
                const done = (state.prayed ?? []).includes(v);
                setSlotVisible(r.prayed, Boolean(passed), done ? 255 : DIM_IDLE);
                r.prayed.accessible_name = s.t('prayed');
                r.prayed.style_class = `sakaleyn-icon-button${done ? ' sakaleyn-done' : ''}`;
            }
        }

        const notices = [];
        if (state.today?.source === 'calculated')
            notices.push(s.t('estimated'));
        const needsLocation = state.location?.status !== 'pending' && Boolean(state.location?.guessed);
        if (state.location?.status === 'pending')
            notices.push(s.t('location_pending'));
        else if (needsLocation)
            notices.push(s.t('location_unavailable'));
        this._notice.text = notices.join('\n');
        this._noticeButton.visible = notices.length > 0;
        this._noticeButton.reactive = needsLocation;
        this._noticeButton.can_focus = needsLocation;
        this._noticeButton.accessible_name = needsLocation ? s.t('choose_location') : '';
        this._noticeButton.style_class = `sakaleyn-notice-button${needsLocation ? ' sakaleyn-notice-action' : ''}`;

        const rel = state.religious_upcoming?.[0];
        this._religious.text = rel && rel.days <= 30 ? `${rel.name} · ${s.inDays(rel.days)}` : '';
        this._religious.visible = Boolean(this._religious.text);

        this._stop.visible = Boolean(state.playing);
        this.tick();
    }

    /** Saniyede bir: büyük sayaç ve yarım daire göstergesi. */
    tick() {
        const state = this._state;
        if (!state)
            return;
        const s = this._ctx.strings();
        const now = Date.now();
        const t = target(state, now);
        if (!t) {
            this._caption.text = '';
            this._big.text = '--:--';
            return;
        }
        if (state.playing?.kind === 'adhan') {
            this._caption.text = s.t('playing_adhan');
        } else if (t.kind === 'iftar') {
            this._caption.text = s.t('until_iftar');
        } else {
            this._caption.text = s.untilVakit(t.vakit);
        }
        this._big.text = s.clock(t.ms);
        const flags = [];
        if (state.ramadan)
            flags.push(s.t('ramadan'));
        if (state.friday)
            flags.push(s.t('friday'));
        if (state.enabled === false)
            flags.push(s.t('notifications_off'));
        else if (state.muted_today)
            flags.push(s.t('muted'));
        const isTomorrow = state.today?.instants?.[t.vakit] && Date.parse(state.today.instants[t.vakit]) < t.at - 12 * 3600000;
        this._sub.text = [isTomorrow ? `${s.t('tomorrow_word')} ${t.local}` : t.local, ...flags].filter(Boolean).join(' · ');

        // şimdiki vaktin başından sıradaki vakte ilerleme
        const since = state.current?.since ? Date.parse(state.current.since) : null;
        const until = state.next?.at ? Date.parse(state.next.at) : null;
        const frac = since && until && until > since ? Math.min(1, Math.max(0, (now - since) / (until - since))) : 0;
        this._fitGauge();
        if (Math.abs(frac - this._frac) > 0.0005) {
            const jump = Math.abs(frac - this._frac) > 0.02; // vakit geçişi ya da ilk veri
            this._frac = frac;
            if (jump)
                this._animateTo(frac);
            else if (!this._timeline) {
                this._shown = frac;
                this._gauge.queue_repaint();
            }
        }
    }

    destroy() {
        this._timeline?.stop();
        this._timeline = null;
        this.actor.destroy();
    }
}
