// Sakaleyn: D-Bus istemcisi. Eklenti, tercihler ve bağımsız ayarlar penceresi
// yalnızca bu servisi konuşur; ses, bildirim ve Diyanet verisi serviste.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

export const BUS_NAME = 'io.github.ensignalp.Sakaleyn';
export const OBJECT_PATH = '/io/github/ensignalp/Sakaleyn';
const SCHEMA_VERSION = 1;
const RETRY_SECONDS = 20;

const Iface = `
<node>
  <interface name="io.github.ensignalp.Sakaleyn1">
    <method name="GetState"><arg type="s" name="state" direction="out"/></method>
    <method name="GetSettings"><arg type="s" name="settings" direction="out"/></method>
    <method name="SetSettings"><arg type="s" name="patch" direction="in"/></method>
    <method name="SearchDistricts">
      <arg type="s" name="query" direction="in"/><arg type="s" name="result" direction="out"/>
    </method>
    <method name="SetDistrict"><arg type="u" name="id" direction="in"/></method>
    <method name="UseAutoLocation"/>
    <method name="RefreshTimes"/>
    <method name="StopAudio"/>
    <method name="MuteToday"><arg type="b" name="muted" direction="in"/></method>
    <method name="Prayed"><arg type="s" name="vakit" direction="in"/></method>
    <method name="PreviewSound"><arg type="s" name="id" direction="in"/></method>
    <method name="ListSounds"><arg type="s" name="sounds" direction="out"/></method>
    <method name="TestEvent">
      <arg type="s" name="kind" direction="in"/><arg type="s" name="vakit" direction="in"/>
    </method>
    <signal name="StateChanged"><arg type="s" name="state"/></signal>
    <signal name="Event"><arg type="s" name="kind"/><arg type="s" name="vakit"/></signal>
    <property name="Version" type="s" access="read"/>
  </interface>
</node>`;

export const SakaleynProxy = Gio.DBusProxy.makeProxyWrapper(Iface);

export function describe(error) {
    if (error instanceof GLib.Error) {
        const missing = error.matches(Gio.DBusError, Gio.DBusError.SERVICE_UNKNOWN) ||
            error.matches(Gio.DBusError, Gio.DBusError.NAME_HAS_NO_OWNER) ||
            error.matches(Gio.DBusError, Gio.DBusError.SPAWN_SERVICE_NOT_FOUND);
        if (Gio.DBusError.is_remote_error(error))
            Gio.DBusError.strip_remote_error(error);
        return {kind: missing ? 'service-missing' : 'dbus', message: error.message};
    }
    return {kind: 'dbus', message: String(error?.message ?? error)};
}

function isCancelled(error) {
    return error instanceof GLib.Error && error.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED);
}

/** Tek seferlik proxy (servis kapalıysa D-Bus etkinleştirmesiyle başlatılır). */
export function newProxy(cancellable = null) {
    return SakaleynProxy.newAsync(Gio.DBus.session, BUS_NAME, OBJECT_PATH, cancellable,
        Gio.DBusProxyFlags.DO_NOT_LOAD_PROPERTIES);
}

/** Proxy üzerinde çağrı; ilk dönüş değerini verir, hatayı okunur mesajla fırlatır. */
export async function call(proxy, method, ...args) {
    try {
        const out = await proxy[`${method}Async`](...args);
        return out?.[0];
    } catch (e) {
        throw new Error(describe(e).message);
    }
}

export class SakaleynClient {
    /**
     * @param {object} handlers
     * @param {(state: object) => void} handlers.onState
     * @param {(error: {kind: string, message: string}) => void} handlers.onError
     */
    constructor({onState, onError}) {
        this._onState = onState;
        this._onError = onError;
        this._cancellable = new Gio.Cancellable();
        this._proxy = null;
        this._signalId = 0;
        this._retryId = 0;
        this._destroyed = false;
    }

    async start() {
        if (this._destroyed || this._proxy)
            return;
        try {
            this._proxy = await newProxy(this._cancellable);
        } catch (e) {
            if (!this._destroyed && !isCancelled(e)) {
                this._onError(describe(e));
                this._scheduleRetry();
            }
            return;
        }
        if (this._destroyed) {
            this._proxy = null;
            return;
        }
        this._signalId = this._proxy.connectSignal('StateChanged', (_p, _s, [json]) => this._deliver(json));
        this._proxy.connectObject('notify::g-name-owner', () => {
            if (this._proxy?.g_name_owner)
                this.fetch();
        }, this);
        await this.fetch();
    }

    async fetch() {
        if (!this._proxy)
            return this.start();
        try {
            const [json] = await this._proxy.GetStateAsync();
            this._deliver(json);
        } catch (e) {
            if (!this._destroyed && !isCancelled(e)) {
                this._onError(describe(e));
                this._scheduleRetry();
            }
        }
        return undefined;
    }

    call(method, ...args) {
        if (!this._proxy)
            return Promise.reject(new Error('service unavailable'));
        return call(this._proxy, method, ...args);
    }

    _deliver(json) {
        let state;
        try {
            state = JSON.parse(json);
        } catch (e) {
            this._onError({kind: 'invalid', message: String(e)});
            return;
        }
        if (state?.schema !== SCHEMA_VERSION) {
            this._onError({kind: 'schema', message: `unsupported state schema ${state?.schema}`});
            return;
        }
        this._clearRetry();
        this._onState(state);
    }

    _scheduleRetry() {
        if (this._retryId || this._destroyed)
            return;
        this._retryId = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, RETRY_SECONDS, () => {
            this._retryId = 0;
            this.fetch();
            return GLib.SOURCE_REMOVE;
        });
    }

    _clearRetry() {
        if (this._retryId) {
            GLib.source_remove(this._retryId);
            this._retryId = 0;
        }
    }

    destroy() {
        this._destroyed = true;
        this._cancellable.cancel();
        this._clearRetry();
        if (this._proxy) {
            if (this._signalId)
                this._proxy.disconnectSignal(this._signalId);
            this._proxy.disconnectObject(this);
        }
        this._proxy = null;
    }
}
