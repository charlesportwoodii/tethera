<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { invoke } from "@tauri-apps/api/core";
  import { Button, Label, NavBar, Toggle } from "$console";
  import { SettingsManager, type Sensor, type SensorState } from "$managers/settings_manager";

  /**
   * The sensor, or nothing at all on a platform without one.
   *
   * Imported at the moment of use rather than at the top, because the plugin
   * exists only in a mobile build and a static import would make a desktop
   * bundle reference a module that is not there.
   */
  async function findSensor(): Promise<Sensor | null> {
    try {
      const biometric = await import("@tauri-apps/plugin-biometric");
      const status = await biometric.checkStatus();

      manager.describeSensor(read(status.isAvailable, status.errorCode));

      // Not gated on `isAvailable`. That reports only the *biometric* half, and
      // a phone with no fingerprint enrolled still has a screen PIN, which
      // `allowDeviceCredential` accepts and which locks the app perfectly well.
      // Refusing here would turn "you have not set up a fingerprint" into "this
      // feature does not work on your phone".
      if (status.errorCode === "passcodeNotSet") {
        return null;
      }

      return async (reason: string) => {
        await biometric.authenticate(reason, {
          // The way back in when the sensor will not read a thumb. Without it a
          // wet finger or a re-enrolled print locks somebody out of the machines
          // they would fix it from.
          allowDeviceCredential: true,
          title: "Tethera",
          confirmationRequired: false,
        });
      };
    } catch {
      manager.describeSensor({
        usable: false,
        note: "This build cannot reach the phone's lock",
      });

      return null;
    }
  }

  /**
   * What to say about the sensor.
   *
   * "No sensor" and "no fingerprint enrolled" are different facts with
   * different remedies, and the second is by far the more common. Telling
   * somebody their phone cannot do this, when what they need is to enrol a
   * finger, sends them to the wrong screen.
   */
  function read(available: boolean, code: string | undefined): SensorState {
    if (available) {
      return {
        usable: true,
        note: "Your fingerprint or face, every time the app comes back",
      };
    }

    if (code === "passcodeNotSet") {
      return {
        usable: false,
        note: "Set a screen lock on this phone first — there is nothing to check against",
      };
    }

    if (code === "biometryNotEnrolled") {
      return {
        usable: true,
        note: "No fingerprint is set up on this phone, so your screen PIN will be asked for instead",
      };
    }

    if (code === "biometryLockout") {
      return {
        usable: true,
        note: "Too many attempts, so your screen PIN will be asked for instead",
      };
    }

    return {
      usable: true,
      note: "This phone has no sensor, so your screen PIN will be asked for instead",
    };
  }

  const manager = new SettingsManager(invoke, findSensor);
  const preferences = manager.preferences;
  const servers = manager.servers;
  const sensor = manager.sensor;
  const busy = manager.busy;
  const error = manager.error;

  let version = $state("");

  onMount(() => {
    void start();
  });

  async function start(): Promise<void> {
    await manager.load();

    try {
      version = (await invoke("app_version")) as string;
    } catch {
      version = "unknown";
    }
  }

  function when(at: number | null | undefined): string {
    if (at === null || at === undefined) {
      return "date unknown";
    }

    return new Date(Number(at)).toLocaleDateString(undefined, {
      day: "numeric",
      month: "short",
    });
  }
</script>

<div class="screen">
  <NavBar title="Settings" subtitle="this phone" onback={() => goto("/")} />

  <div class="body">
    <Label kind="section">locking</Label>
    <div class="node">
      <span class="what">
        <b>Ask for me when this opens</b>
        <span class="why">{$sensor.note}</span>
      </span>
      <Toggle
        label="Ask for me when this opens"
        checked={$preferences.biometric_lock}
        disabled={!$sensor.usable || $busy}
        onchange={(on) => void manager.setLock(on)}
      />
    </div>
    <p class="plain">
      A lock on the door, not a safe. It stops somebody who is holding your unlocked phone. It
      does not protect the key itself, which stays readable to anything running inside this app.
    </p>

    <Label kind="section">servers</Label>
    {#each $servers as row (row.entry.server.id)}
      <div class="node row">
        <span class="what">
          <b>{row.entry.server.label ?? row.entry.server.id}</b>
          <span class="why">paired {when(row.entry.device.paired_at)}</span>
        </span>
        <Button
          variant="quiet"
          onclick={() => void manager.forget(row.entry.server.id as unknown as string)}
        >
          forget
        </Button>
      </div>
    {:else}
      <p class="plain">No machines yet.</p>
    {/each}
    <div class="node end">
      <Button icon="scan" onclick={() => goto("/pair")}>Add a server</Button>
    </div>

    <Label kind="section">this device</Label>
    <div class="node">
      <span class="what"><b>Version</b></span>
      <span class="value">{version}</span>
    </div>

    {#if $error}
      <p class="fault">{$error}</p>
    {/if}
  </div>
</div>

<style lang="scss">
  .screen {
    display: flex;
    flex-direction: column;
    height: 100dvh;
    overflow: hidden;
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 6px 14px 24px;
  }

  // Flat, because nothing in settings nests. The elbows the rest of the app
  // uses would be decoration here.
  .node {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 12px 0;
    border-bottom: 1px solid var(--tc-rule);

    &.end {
      justify-content: flex-start;
      border-bottom: none;
    }
  }

  .what {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;

    b {
      font-size: 13.5px;
      font-weight: 600;
      color: var(--tc-ink-1);
    }
  }

  .why {
    font-size: 11.5px;
    line-height: 1.4;
    color: var(--tc-ink-3);
  }

  // The name may wrap; the date beside a control must not, or it pushes into
  // the control and the row reads as two broken columns.
  .row .why {
    white-space: nowrap;
  }

  .value {
    flex: none;
    font-family: var(--tc-mono);
    font-size: 11.5px;
    color: var(--tc-ink-2);
  }

  .plain {
    margin: 10px 0 0;
    font-size: 11.5px;
    line-height: 1.5;
    color: var(--tc-ink-3);
  }

  .fault {
    margin: 14px 0 0;
    font-size: 12px;
    line-height: 1.5;
    color: var(--tc-bad);
  }
</style>
