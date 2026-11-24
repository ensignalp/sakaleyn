// Sakaleyn: Hızlı Ayarlar kutucuğu. Basit aç/kapa: açıkken vakit uyarıları,
// ezan ve bildirimler etkin; kapatınca hiçbiri gelmez (servisteki `enabled`).
// Alt yazı sıradaki vakti ve kalan süreyi gösterir.

import GObject from 'gi://GObject';

import {QuickToggle, SystemIndicator} from 'resource:///org/gnome/shell/ui/quickSettings.js';

import {sakaleynGicon, target} from './popup.js';

const SakaleynToggle = GObject.registerClass(
class SakaleynToggle extends QuickToggle {
    _init(ctx) {
        super._init({title: 'Sakaleyn', toggleMode: false, gicon: sakaleynGicon(ctx.extPath)});
        this._ctx = ctx;
        this._state = null;
        this.connect('clicked', () => ctx.actions.setEnabled(!this.checked));
    }

    update(state) {
        this._state = state;
        const s = this._ctx.strings();
        const on = Boolean(state && state.enabled !== false);
        this.checked = on;
        const t = target(state);
        if (!state)
            this.subtitle = '';
        else if (!on)
            this.subtitle = s.t('notifications_off');
        else
            this.subtitle = t ? `${s.vakit(t.vakit)} ${t.local} · ${s.countdown(t.ms)}` : '';
        this.accessible_name = `Sakaleyn: ${on ? s.t('notifications_on') : s.t('notifications_off')}`;
    }

    tick() {
        if (this._state)
            this.update(this._state);
    }
});

export const SakaleynQuickSettings = GObject.registerClass(
class SakaleynQuickSettings extends SystemIndicator {
    _init(ctx) {
        super._init();
        this._toggle = new SakaleynToggle(ctx);
        this.quickSettingsItems.push(this._toggle);
    }

    attach(qs) {
        qs.addExternalIndicator(this);
    }

    update(state) {
        this._toggle.update(state);
    }

    tick() {
        this._toggle.tick();
    }

    destroy() {
        this.quickSettingsItems.forEach(i => i.destroy());
        this.quickSettingsItems.length = 0;
        super.destroy();
    }
});
