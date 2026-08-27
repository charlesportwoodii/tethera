// Tauri has no Node server, so the app prerenders to static files.
// See https://v2.tauri.app/start/frontend/sveltekit/
import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter(),
    alias: {
      // The Console design system. Kit turns this into a tsconfig path too, so
      // `import { StatusGlyph } from "$console"` resolves for svelte-check as
      // well as for the bundler.
      $console: "src/console",
      $bindings: "src/js/bindings",
      // Not `$app`: SvelteKit already owns that prefix for `$app/navigation`
      // and friends, and shadowing it breaks the framework's own imports.
      $managers: "src/js/app",
      $components: "src/js/components",
    },
  },
};

export default config;
