import { writable, type Readable, type Writable } from "svelte/store";
import type { Preferences } from "$bindings/Preferences";
import type { ServerRow } from "$bindings/ServerRow";
import type { Invoke } from "./server_manager";

/** What the platform will do when asked to identify somebody. */
export type Sensor = (reason: string) => Promise<void>;

/**
 * Finds the sensor, or answers that there is none.
 *
 * A function rather than a `Sensor`, because the plugin exists only in a mobile
 * build and finding it is asynchronous. Resolving it before construction would
 * make this manager async to build, and a store cannot be subscribed to before
 * it exists - which is what puts a screen in the position of drawing nothing
 * while it waits.
 */
export type SensorSource = () => Promise<Sensor | null>;

/**
 * What this phone can do when asked to identify somebody, and what to say
 * about it.
 *
 * Three states rather than two, because "no sensor" and "no fingerprint
 * enrolled" are different facts with different remedies, and reporting the
 * second as the first tells somebody their phone cannot do a thing it does
 * perfectly well. Only one state genuinely stops the lock: a phone with no
 * screen lock at all has nothing to fall back to.
 */
export interface SensorState {
  usable: boolean;
  note: string;
}

const NOTHING: Preferences = { biometric_lock: false };

/**
 * This device's own settings, and the lock that guards every machine it knows.
 *
 * The lock state is never held here as the truth. It is read from the Rust side
 * on every check, because that is the flag `AppState::connect` consults, and a
 * copy kept in the webview would be a second answer to the same question - the
 * kind that drifts and then disagrees at the worst moment.
 */
export class SettingsManager {
  private readonly preferenceStore: Writable<Preferences>;
  private readonly serverStore: Writable<ServerRow[]>;
  private readonly lockedStore: Writable<boolean>;
  private readonly busyStore: Writable<boolean>;
  private readonly errorStore: Writable<string | null>;
  private readonly sensorStore: Writable<SensorState>;

  public readonly preferences: Readable<Preferences>;
  public readonly servers: Readable<ServerRow[]>;
  public readonly locked: Readable<boolean>;
  public readonly busy: Readable<boolean>;
  public readonly error: Readable<string | null>;
  /** What this phone can do when asked to identify somebody. */
  public readonly sensor: Readable<SensorState>;

  private authenticator: Sensor | null = null;

  constructor(
    private readonly invoke: Invoke,
    private readonly source: SensorSource,
  ) {
    this.preferenceStore = writable(NOTHING);
    this.serverStore = writable([]);
    this.lockedStore = writable(false);
    this.busyStore = writable(false);
    this.errorStore = writable(null);
    this.sensorStore = writable({ usable: false, note: "Checking this phone…" });

    this.preferences = { subscribe: this.preferenceStore.subscribe };
    this.servers = { subscribe: this.serverStore.subscribe };
    this.locked = { subscribe: this.lockedStore.subscribe };
    this.busy = { subscribe: this.busyStore.subscribe };
    this.error = { subscribe: this.errorStore.subscribe };
    this.sensor = { subscribe: this.sensorStore.subscribe };
  }

  /** Records what the platform said it can do. */
  describeSensor(state: SensorState): void {
    this.sensorStore.set(state);
  }

  async load(): Promise<void> {
    try {
      this.authenticator = await this.source();
    } catch {
      this.authenticator = null;
    }

    try {
      const [preferences, servers] = await Promise.all([
        this.invoke("read_preferences") as Promise<Preferences>,
        this.invoke("list_servers") as Promise<ServerRow[]>,
      ]);

      this.preferenceStore.set(preferences ?? NOTHING);
      this.serverStore.set(servers ?? []);
    } catch (error) {
      this.errorStore.set(String(error));
    }
  }

  /**
   * Turns the launch lock on or off, proving who this is first.
   *
   * The proof is required in both directions, and turning it *off* is the
   * direction that matters: without it, whoever is holding the unlocked phone
   * could simply switch off the lock that was meant to stop them.
   */
  async setLock(on: boolean): Promise<boolean> {
    if (!this.authenticator) {
      this.errorStore.set("nothing on this phone can confirm who you are");

      return false;
    }

    this.errorStore.set(null);
    this.busyStore.set(true);

    try {
      await this.authenticator(on ? "Confirm it is you before locking this app" : "Confirm it is you to remove the lock");

      const updated = (await this.invoke("set_biometric_lock", { on })) as Preferences;
      this.preferenceStore.set(updated);

      return true;
    } catch (error) {
      // A refused or failed authentication is an ordinary outcome, not a fault.
      // The setting simply does not change, and the switch springs back.
      this.errorStore.set(this.readable(error));

      return false;
    } finally {
      this.busyStore.set(false);
    }
  }

  /** Asks the Rust side whether anything may reach a machine right now. */
  async refreshLock(): Promise<boolean> {
    try {
      const unlocked = (await this.invoke("is_unlocked")) as boolean;
      this.lockedStore.set(!unlocked);

      return !unlocked;
    } catch {
      // Unreadable means locked. The safe direction is the one that asks again.
      this.lockedStore.set(true);

      return true;
    }
  }

  /**
   * Authenticates and opens the door.
   *
   * Returns false rather than throwing so a lock screen can simply stay up. A
   * failed attempt is the normal case here - a thumb in the wrong place - and
   * treating it as an error would put a fault message over a screen whose only
   * job is to be tried again.
   */
  async unlock(): Promise<boolean> {
    if (!this.authenticator) {
      return false;
    }

    this.errorStore.set(null);

    try {
      await this.authenticator("Unlock Tethera");
      await this.invoke("unlock");
      this.lockedStore.set(false);

      return true;
    } catch (error) {
      this.errorStore.set(this.readable(error));

      return false;
    }
  }

  /** Closes the door, which is what leaving the app does. */
  async lock(): Promise<void> {
    try {
      await this.invoke("lock");
      await this.refreshLock();
    } catch {
      // Nothing to do. The gate is the Rust flag, and a failure to set it
      // leaves it as it was rather than opening it.
    }
  }

  async forget(id: string): Promise<void> {
    try {
      await this.invoke("forget_server", { id });
      await this.load();
    } catch (error) {
      this.errorStore.set(String(error));
    }
  }

  private readable(error: unknown): string {
    const text = String(error);

    // The plugin reports a cancelled prompt as an error. It is not one, and a
    // person who changed their mind should not be told something failed.
    if (/cancel|userCancel|dismiss/i.test(text)) {
      return "";
    }

    return text;
  }
}
