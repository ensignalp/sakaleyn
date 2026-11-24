// Sakaleyn: tarih/saat penceresinde (takvimin yanında) bugünün vakitleri kartı.
// Hava durumu kartıyla aynı görünüm için onun sınıfları kullanılır. Açılır
// pencereyle aynı dil: şimdiki vakit dolgulu, sıradaki çerçeveli, geçmiş soluk;
// başlıkta sıradaki vakit ve canlı geri sayım, altta şimdiki vaktin ilerlemesi.

import Clutter from 'gi://Clutter';
import Pango from 'gi://Pango';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {VAKITS} from './i18n.js';
import {target} from './popup.js';

const VERTICAL = Clutter.Orientation.VERTICAL;

export class SakaleynDateMenu {
    constructor(ctx) {
        this._ctx = ctx;
        this._state = null;
        this.actor = new St.Button({style_class: 'weather-button sakaleyn-datemenu', x_expand: true, can_focus: true});
        const box = new St.BoxLayout({orientation: VERTICAL, x_expand: true, style_class: 'sakaleyn-dm-box'});

        const head = new St.BoxLayout({x_expand: true, style_class: 'sakaleyn-dm-head'});
        head.add_child(new St.Icon({icon_name: 'mark-location-symbolic', icon_size: 14, style_class: 'sakaleyn-place-icon',
            y_align: Clutter.ActorAlign.CENTER}));
        this._title = new St.Label({style_class: 'weather-header sakaleyn-dm-title', x_expand: true,
            y_align: Clutter.ActorAlign.CENTER});
        this._next = new St.Label({style_class: 'sakaleyn-dm-next', y_align: Clutter.ActorAlign.CENTER});
        this._left = new St.Label({style_class: 'sakaleyn-dm-left', y_align: Clutter.ActorAlign.CENTER});
        head.add_child(this._title);
        head.add_child(this._next);
        head.add_child(this._left);
        box.add_child(head);

        this._grid = new St.BoxLayout({style_class: 'sakaleyn-dm-grid', x_expand: true});
        this._cells = {};
        for (const v of VAKITS) {
            const cell = new St.BoxLayout({orientation: VERTICAL, x_expand: true, style_class: 'sakaleyn-dm-cell'});
            const name = new St.Label({style_class: 'sakaleyn-dm-name', x_align: Clutter.ActorAlign.CENTER});
            const time = new St.Label({style_class: 'sakaleyn-dm-time', x_align: Clutter.ActorAlign.CENTER});
            time.clutter_text.ellipsize = Pango.EllipsizeMode.NONE; // saat hiçbir zaman kısalmaz
            cell.add_child(name);
            cell.add_child(time);
            this._cells[v] = {cell, name, time};
            this._grid.add_child(cell);
        }
        box.add_child(this._grid);

        this._track = new St.Widget({style_class: 'sakaleyn-dm-track', x_expand: true, layout_manager: new Clutter.BinLayout()});
        this._fill = new St.Widget({style_class: 'sakaleyn-dm-fill', x_align: Clutter.ActorAlign.START, width: 0});
        this._track.add_child(this._fill);
        box.add_child(this._track);

        this.actor.set_child(box);
        this.actor.connect('clicked', () => {
            Main.panel.statusArea.dateMenu?.menu.close();
            ctx.actions.openPopup();
        });
        const dm = Main.panel.statusArea.dateMenu;
        const parent = dm?._weatherItem?.get_parent();
        if (parent)
            parent.insert_child_above(this.actor, dm._weatherItem);
    }

    update(state) {
        this._state = state;
        const s = this._ctx.strings();
        this.actor.visible = Boolean(state?.today);
        this._title.text = state?.district?.name ?? s.t('prayer_times');
        const current = state?.current?.vakit;
        const nextToday = state?.next && state.today?.instants?.[state.next.vakit] &&
            Date.parse(state.today.instants[state.next.vakit]) === Date.parse(state.next.at);
        for (const v of VAKITS) {
            const c = this._cells[v];
            c.name.text = s.vakit(v);
            c.time.text = state?.today?.times?.[v] ?? '--:--';
            const passed = state?.today?.instants?.[v] && Date.parse(state.today.instants[v]) <= Date.now();
            c.cell.style_class = `sakaleyn-dm-cell${v === current ? ' sakaleyn-current' : ''}` +
                `${nextToday && v === state.next.vakit ? ' sakaleyn-next' : ''}${passed && v !== current ? ' sakaleyn-past' : ''}`;
        }
        this.tick();
    }

    /** Saniyede bir (kart görünürken): geri sayım ve ilerleme çizgisi. */
    tick() {
        const state = this._state;
        if (!state || !this.actor.mapped)
            return;
        const s = this._ctx.strings();
        const t = target(state);
        this._next.text = t ? `${t.kind === 'iftar' ? s.t('until_iftar') : s.vakit(t.vakit)} ${t.local}` : '';
        this._left.text = t ? s.countdown(t.ms) : '';
        const since = state.current?.since ? Date.parse(state.current.since) : null;
        const until = state.next?.at ? Date.parse(state.next.at) : null;
        const frac = since && until && until > since ? Math.min(1, Math.max(0, (Date.now() - since) / (until - since))) : 0;
        this._fill.width = Math.round(this._track.width * frac);
    }

    destroy() {
        this.actor.destroy();
    }
}
