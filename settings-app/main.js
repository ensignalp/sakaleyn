// Sakaleyn: bağımsız ayarlar penceresi (KDE, XFCE, Cinnamon, MATE…).
// Kurulumda eklentideki settings-ui.js ve lib/ ile aynı klasöre kopyalanır:
//   …/share/sakaleyn/settings/{main.js, settings-ui.js, lib/client.js, lib/i18n.js}
// Çalıştırma: `sakaleyn settings` ya da `gjs -m …/main.js`.

import Adw from 'gi://Adw?version=1';
import Gtk from 'gi://Gtk?version=4.0';
import GLib from 'gi://GLib';
import {programArgs} from 'system';

import {fillWindow} from './settings-ui.js';

GLib.set_prgname('sakaleyn-settings');
GLib.set_application_name('Sakaleyn');

const app = new Adw.Application({application_id: 'io.github.ensignalp.Sakaleyn.Settings'});
app.connect('activate', async () => {
    const existing = app.get_active_window();
    if (existing) {
        existing.present();
        return;
    }
    const win = new Adw.PreferencesWindow({application: app, title: 'Sakaleyn'});
    Gtk.Window.set_default_icon_name('sakaleyn');
    try {
        await fillWindow(win, null);
    } catch (e) {
        logError(e, 'Sakaleyn settings');
    }
    win.present();
});
app.run([imports.system.programInvocationName, ...programArgs]);
