<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  // Proves the Tauri command boundary end to end: the string below is produced
  // by AppState in the Rust process, not by anything in the bundle.
  let appVersion = $state("loading");

  invoke<string>("app_version")
    .then((v) => (appVersion = v))
    .catch((e) => (appVersion = `error: ${e}`));
</script>

<main>
  <h1>Tethera</h1>
  <p>{appVersion}</p>
</main>
