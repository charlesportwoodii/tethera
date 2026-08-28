#!/usr/bin/env node
/**
 * Copies `client/src-tauri/android/MainActivity.kt` over the one
 * `tauri android init` generated.
 *
 * `gen/android/` is regenerated on every init and is gitignored, so the
 * committed activity reaches a build only by being copied in afterwards. Nothing
 * did that before this script existed, and the stock activity Tauri writes
 * builds and launches — it just does not carry the edge-to-edge and WebView
 * setup, so the difference is a layout that looks wrong rather than a build
 * that fails.
 *
 * Must run AFTER `tauri android init`.
 *
 * Node rather than `cp`: `mise run build-android` runs this on Windows, where
 * the shell is PowerShell and the path separators differ.
 */
const fs = require('fs');
const path = require('path');

const rootDir = path.resolve(__dirname, '../..');
const source = path.join(rootDir, 'client/src-tauri/android/MainActivity.kt');
const conf = path.join(rootDir, 'client/src-tauri/tauri.conf.json');

if (!fs.existsSync(source)) {
  console.error(`Not found: ${source}`);
  process.exit(1);
}

// Derived from the identifier rather than written down twice. Tauri lays the
// generated sources out under the identifier's package path, so a change to
// `identifier` moves the file this has to overwrite.
const { identifier } = JSON.parse(fs.readFileSync(conf, 'utf8'));

if (!identifier) {
  console.error(`No "identifier" in ${conf}.`);
  process.exit(1);
}

const target = path.join(
  rootDir,
  'client/src-tauri/gen/android/app/src/main/java',
  ...identifier.split('.'),
  'MainActivity.kt',
);

if (!fs.existsSync(path.dirname(target))) {
  console.error(`Not found: ${path.dirname(target)}`);
  console.error('Run `yarn tauri android init` before this script.');
  process.exit(1);
}

fs.copyFileSync(source, target);

console.log(`Installed ${source}`);
console.log(`       to ${target}`);
