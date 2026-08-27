import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { fileURLToPath } from "node:url";

// The design system is framework code with no SvelteKit runtime in the picture,
// so the plain svelte plugin is enough here — kit's plugin would pull in a
// router these components never touch.
export default defineConfig({
  plugins: [svelte()],
  resolve: {
    alias: {
      $console: fileURLToPath(new URL("./src/console", import.meta.url)),
      $bindings: fileURLToPath(new URL("./src/js/bindings", import.meta.url)),
      $managers: fileURLToPath(new URL("./src/js/app", import.meta.url)),
      $components: fileURLToPath(new URL("./src/js/components", import.meta.url)),
      $lib: fileURLToPath(new URL("./src/lib", import.meta.url)),
    },
    // Without this Svelte resolves to its server build and mount() throws:
    // happy-dom is a DOM, so the browser entry is the correct one.
    conditions: ["browser"],
  },
  test: {
    environment: "happy-dom",
    setupFiles: ["./src/test/setup.ts"],
    include: ["src/**/*.test.ts"],
    exclude: ["node_modules/**"],
    coverage: {
      provider: "v8",
      reporter: ["text-summary", "lcov"],
      reportsDirectory: "./coverage",
      // Reported whether or not a test touched them, so an untested component
      // shows at zero rather than being absent.
      all: true,
      include: ["src/console/**/*.{ts,svelte}"],
      exclude: [
        // Generated from the Rust types in common/. The generator is what is
        // under test, not its output.
        "src/js/bindings/**",
        "src/console/**/index.ts",
        "src/**/*.d.ts",
      ],
    },
  },
});
