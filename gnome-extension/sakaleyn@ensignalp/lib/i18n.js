// Sakaleyn: metinler (Türkçe / İngilizce) ve zaman biçimleri.
// Eklenti, tercihler ve bağımsız ayarlar penceresi ortak kullanır (Shell'e bağımlı değil).

export const VAKITS = ['imsak', 'gunes', 'ogle', 'ikindi', 'aksam', 'yatsi'];
export const ADHAN_VAKITS = ['imsak', 'ogle', 'ikindi', 'aksam', 'yatsi'];

const NAMES = {
    tr: {imsak: 'İmsak', gunes: 'Güneş', ogle: 'Öğle', ikindi: 'İkindi', aksam: 'Akşam', yatsi: 'Yatsı'},
    en: {imsak: 'Fajr', gunes: 'Sunrise', ogle: 'Dhuhr', ikindi: 'Asr', aksam: 'Maghrib', yatsi: 'Isha'},
};

const WEEKDAYS = {
    tr: ['Pazartesi', 'Salı', 'Çarşamba', 'Perşembe', 'Cuma', 'Cumartesi', 'Pazar'],
    en: ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'],
};
const MONTHS = {
    tr: ['Ocak', 'Şubat', 'Mart', 'Nisan', 'Mayıs', 'Haziran', 'Temmuz', 'Ağustos', 'Eylül', 'Ekim', 'Kasım', 'Aralık'],
    en: ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October',
        'November', 'December'],
};

const T = {
    // eklenti
    service_missing: ['Sakaleyn servisi çalışmıyor', 'Sakaleyn service is not running'],
    service_missing_body: ['`sakaleyn` paketini kurun ya da `systemctl --user start sakaleyn` çalıştırın.',
        'Install the `sakaleyn` package or run `systemctl --user start sakaleyn`.'],
    waiting: ['Bekleniyor…', 'Waiting…'],
    until: ['vaktine', 'in'],
    until_iftar: ['İftara', 'Iftar in'],
    until_imsak: ['İmsaka', 'Fajr in'],
    estimated: ['Tahmini vakitler: Diyanet\'e ulaşılamadı', 'Estimated times: Diyanet unreachable'],
    location_pending: ['Konum bulunuyor…', 'Finding location…'],
    location_unavailable: ['Konum bulunamadı; ayarlardan ilçenizi seçin', 'Location unavailable; choose your district in settings'],
    location_guess: ['Tahmini konum', 'Guessed location'],
    stop: ['Sustur', 'Stop'],
    muted: ['Bugün sessiz', 'Silent today'],
    notifications_on: ['Bildirimler açık', 'Notifications on'],
    notifications_off: ['Bildirimler kapalı', 'Notifications off'],
    choose_location: ['Konum seç', 'Choose location'],
    settings: ['Ayarlar', 'Settings'],
    prayed: ['Kıldım', 'Prayed'],
    adhan_on: ['Ezan açık', 'Adhan on'],
    adhan_off: ['Ezan kapalı', 'Adhan off'],
    sunrise_no_adhan: ['Güneş vaktinde namaz ve ezan yok', 'No prayer or adhan at sunrise'],
    playing_adhan: ['Ezan okunuyor', 'Adhan playing'],
    in_days: ['gün sonra', 'days'],
    today_word: ['bugün', 'today'],
    tomorrow_word: ['yarın', 'tomorrow'],
    prayer_times: ['Namaz vakitleri', 'Prayer times'],
    friday: ['Cuma', 'Friday'],
    ramadan: ['Ramazan', 'Ramadan'],
    // ayarlar
    page_location: ['Konum', 'Location'],
    page_alerts: ['Uyarılar', 'Reminders'],
    page_adhan: ['Ezan', 'Adhan'],
    page_more: ['Diğer', 'More'],
    page_appearance: ['Görünüm', 'Appearance'],
    auto_location: ['Otomatik konum', 'Automatic location'],
    auto_location_sub: ['GeoClue ile en yakın Diyanet ilçesi; konum hiçbir yere gönderilmez',
        'Nearest Diyanet district via GeoClue; your location is never sent anywhere'],
    current_district: ['Seçili ilçe', 'Current district'],
    search_district: ['İlçe ara (ör. kadıköy, berlin)', 'Search district (e.g. kadıköy, berlin)'],
    search_hint: ['Bir ilçe seçmek otomatik konumu kapatır.', 'Choosing a district turns automatic location off.'],
    refresh_times: ['Vakitleri yenile', 'Refresh times'],
    last_update: ['Son güncelleme', 'Last update'],
    alerts_enabled: ['Vakitten önce uyar', 'Remind before prayer time'],
    alerts_minutes: ['Kaç dakika önce', 'Minutes before'],
    alerts_minutes_sub: ['Bütün vakitler için varsayılan', 'Default for all prayers'],
    per_vakit: ['Vakit başına', 'Per prayer'],
    per_vakit_sub: ['Kapalı vakitte uyarı gelmez; süreyi değiştirmek yalnızca o vakti etkiler',
        'No reminder when off; changing the minutes affects only that prayer'],
    sound: ['Ses', 'Sound'],
    no_sound: ['Sessiz (yalnızca bildirim)', 'Silent (notification only)'],
    volume: ['Ses seviyesi', 'Volume'],
    play_in_dnd: ['Rahatsız Etmeyin açıkken de çal', 'Play in Do Not Disturb'],
    snooze: ['Erteleme (dakika)', 'Snooze (minutes)'],
    test: ['Dene', 'Test'],
    preview: ['Dinle', 'Preview'],
    adhan_enabled: ['Vaktinde ezan oku', 'Play the adhan at prayer time'],
    adhan_vakit_sub: ['Kapalıysa yalnızca bildirim gelir', 'Notification only when off'],
    pause_media: ['Ezan boyunca medyayı duraklat', 'Pause media during the adhan'],
    pause_media_sub: ['Spotify, tarayıcı, VLC… ezan bitince devam eder', 'Spotify, browser, VLC… resume afterwards'],
    add_sounds: ['Kendi sesini ekle', 'Add your own sounds'],
    add_sounds_sub: ['Klasöre mp3/ogg/flac koyun; listede görünür', 'Put mp3/ogg/flac files in this folder'],
    open_folder: ['Klasörü aç', 'Open folder'],
    friday_title: ['Cuma', 'Friday'],
    friday_enabled: ['Cuma namazı hatırlatması', 'Jumu\'ah reminder'],
    friday_minutes: ['Öğleden kaç dakika önce', 'Minutes before Dhuhr'],
    friday_sound: ['Cuma öğle ezanı', 'Friday Dhuhr adhan'],
    same_as_ogle: ['Normal öğle ezanı', 'Same as Dhuhr'],
    ramadan_title: ['Ramazan', 'Ramadan'],
    ramadan_enabled: ['Ramazan modu', 'Ramadan mode'],
    ramadan_enabled_sub: ['Hicri takvimden otomatik: iftar/imsak geri sayımı, sahur uyarısı',
        'Automatic from the Hijri calendar: iftar/fajr countdown, suhoor reminder'],
    sahur_minutes: ['Sahur uyarısı: imsaktan kaç dakika önce', 'Suhoor reminder: minutes before Fajr'],
    sahur_sound: ['Sahur sesi', 'Suhoor sound'],
    extra_title: ['Ek hatırlatmalar', 'Extra reminders'],
    sunrise_alert: ['Güneş doğmadan uyar', 'Remind before sunrise'],
    sunrise_alert_sub: ['Sabah namazı vakti çıkmadan', 'Before Fajr time ends'],
    kerahat: ['Kerahat vakitleri', 'Makruh times'],
    kerahat_sub: ['Güneş doğunca ve akşamdan 45 dk önce bildirim', 'Notify at sunrise and 45 min before Maghrib'],
    end_reminder: ['Vakit çıkıyor hatırlatması', 'Prayer time ending reminder'],
    end_reminder_sub: ['"Kıldım" denmediyse, vakit çıkmadan', 'Unless marked as prayed, before the time ends'],
    religious_days: ['Kandil ve bayram bildirimleri', 'Religious days'],
    offsets: ['Vakit düzeltmeleri (dakika)', 'Time adjustments (minutes)'],
    offsets_sub: ['Diyanet vakitlerine eklenir; genelde gerekmez', 'Added to Diyanet times; rarely needed'],
    language: ['Dil', 'Language'],
    lang_tr: ['Türkçe', 'Turkish'],
    lang_en: ['İngilizce', 'English'],
    lang_auto: ['Sistem dili', 'System language'],
    panel_title: ['Üst panel', 'Top bar'],
    panel_enabled: ['Üst panelde göster', 'Show in the top bar'],
    panel_position: ['Konum', 'Position'],
    pos_left: ['Sol', 'Left'],
    pos_center: ['Orta', 'Center'],
    pos_right: ['Sağ', 'Right'],
    panel_index: ['Sıra', 'Order'],
    panel_format: ['Gösterilen', 'Shows'],
    fmt_remaining: ['Kalan süre', 'Time remaining'],
    fmt_time: ['Vakit saati', 'Prayer time'],
    fmt_both: ['İkisi', 'Both'],
    fmt_icon: ['Yalnızca simge', 'Icon only'],
    panel_show_icon: ['Simge', 'Icon'],
    panel_show_name: ['Vakit adı', 'Prayer name'],
    panel_highlight: ['Vakit yaklaşınca renklendir', 'Highlight when prayer time is near'],
    qs_enabled: ['Hızlı Ayarlar kutucuğu', 'Quick Settings tile'],
    datemenu_enabled: ['Tarih penceresinde vakitler', 'Prayer times in the date menu'],
    about: ['Hakkında', 'About'],
    data_sources: ['Vakitler: Diyanet İşleri Başkanlığı. Konum veritabanı: GeoNames (CC BY 4.0), OpenStreetMap (ODbL).',
        'Prayer times: Presidency of Religious Affairs (Diyanet). Location data: GeoNames (CC BY 4.0), OpenStreetMap (ODbL).'],
    service_version: ['Servis sürümü', 'Service version'],
    test_notification: ['Bildirimi dene', 'Test notification'],
    test_alert: ['Ön uyarıyı dene', 'Test reminder'],
    test_adhan: ['Ezanı dene', 'Test adhan'],
};

export class Strings {
    constructor(lang = 'tr') {
        this.lang = lang === 'en' ? 'en' : 'tr';
    }

    t(key) {
        const v = T[key];
        if (!v)
            return key;
        return this.lang === 'en' ? v[1] : v[0];
    }

    vakit(key) {
        return NAMES[this.lang][key] ?? key;
    }

    /** "Pazartesi, 23 Kasım 2026" (`date`: "2026-11-23"). */
    longDate(date, weekday) {
        const [y, m, d] = date.split('-').map(Number);
        const wd = WEEKDAYS[this.lang][weekday ?? 0];
        return this.lang === 'en' ? `${wd}, ${MONTHS.en[m - 1]} ${d}, ${y}` : `${wd}, ${d} ${MONTHS.tr[m - 1]} ${y}`;
    }

    /** Panel geri sayımı: "1:23", "0:45"; son dakikada "45 sn". */
    countdown(ms) {
        const s = Math.max(0, Math.floor(ms / 1000));
        if (s < 60)
            return this.lang === 'en' ? `${s} s` : `${s} sn`;
        const m = Math.ceil(s / 60);
        return `${Math.floor(m / 60)}:${String(m % 60).padStart(2, '0')}`;
    }

    /** Açılır penceredeki büyük sayaç: "1:23:45". */
    clock(ms) {
        const s = Math.max(0, Math.floor(ms / 1000));
        const h = Math.floor(s / 3600);
        const m = Math.floor((s % 3600) / 60);
        return `${h}:${String(m).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`;
    }

    /** "Akşam vaktine" / "Maghrib in". */
    untilVakit(key) {
        return this.lang === 'en' ? `${this.vakit(key)} in` : `${this.vakit(key)} vaktine`;
    }

    /** "İstanbul vakitleri" / "Times for İstanbul". */
    timesOf(name) {
        return this.lang === 'en' ? `Times for ${name}` : `${name} vakitleri`;
    }

    inDays(n) {
        if (n === 0)
            return this.t('today_word');
        if (n === 1)
            return this.t('tomorrow_word');
        return this.lang === 'en' ? `in ${n} days` : `${n} gün sonra`;
    }
}

export function langOf(state) {
    return state?.lang === 'en' ? 'en' : 'tr';
}
