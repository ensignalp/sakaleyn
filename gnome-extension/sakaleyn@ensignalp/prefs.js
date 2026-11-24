// Sakaleyn: eklenti tercihleri. Arayüzün kendisi settings-ui.js'te; aynı
// dosya diğer masaüstlerinde bağımsız pencere olarak da kullanılır.

import {ExtensionPreferences} from 'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';

import {fillWindow} from './settings-ui.js';

export default class SakaleynPreferences extends ExtensionPreferences {
    fillPreferencesWindow(window) {
        return fillWindow(window, this.getSettings());
    }
}
