// Names what is missing before a build turns it into an obscure error.
// Node rather than shell: mise runs task bodies through cmd on Windows, and the
// `bash` on PATH there is WSL's, which cannot see the Windows toolchain at all.
import { existsSync } from "node:fs";
import { delimiter, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("../", import.meta.url)));

class Preflight {
  static onPath(name) {
    const exts =
      process.platform === "win32"
        ? (process.env.PATHEXT ?? ".EXE;.CMD;.BAT").split(";")
        : [""];

    for (const dir of (process.env.PATH ?? "").split(delimiter)) {
      if (dir.length === 0) continue;
      for (const ext of exts) {
        if (existsSync(join(dir, name + ext))) return true;
      }
    }
    return false;
  }

  static run() {
    const missing = [];
    const warnings = [];

    // herdr is optional at build time: without it the server builds and runs,
    // it just cannot drive a terminal. Everything else below stops a build.
    const commands = [
      ["cargo", "cargo", missing],
      ["yarn", "yarn (run: mise install)", missing],
      ["herdr", "herdr (server will not drive terminals)", warnings],
    ];
    for (const [name, label, bucket] of commands) {
      if (!Preflight.onPath(name)) bucket.push(label);
    }

    const dirs = [
      ["client/node_modules", "client/node_modules (run: mise run deps)"],
      ["client/.svelte-kit", "client/.svelte-kit (run: mise run deps)"],
      ["client/build", "client/build (run: mise run frontend)"],
      ["client/src-tauri/icons", "client/src-tauri/icons (run: yarn tauri icon)"],
    ];
    for (const [path, label] of dirs) {
      if (!existsSync(resolve(root, path))) missing.push(label);
    }

    for (const label of warnings) console.log(`WARNING: ${label}`);
    for (const label of missing) console.log(`MISSING: ${label}`);
    console.log(
      missing.length ? `preflight failed: ${missing.length} missing` : "preflight ok",
    );

    process.exitCode = missing.length ? 1 : 0;
  }
}

Preflight.run();
