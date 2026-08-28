#!/usr/bin/env node
/**
 * Copies `client/src-tauri/Info.ios.plist` over the Info.plist that
 * `tauri ios init` generated.
 *
 * `gen/apple/` is regenerated on every init and is gitignored, so the committed
 * plist reaches a build only by being copied in afterwards. Nothing did that
 * before this script existed: the file sat in the repo describing camera and
 * local-network permissions the built app never carried, and both failures show
 * up on a device rather than in the build.
 *
 * Must run AFTER `tauri ios init` and BEFORE patch-apple-plists.js, which
 * writes the version numbers into whatever plist is in place.
 *
 * Node rather than `cp`: the generated directory is named after the Cargo
 * package, and `mise run` bodies also have to work on Windows.
 */
const fs = require('fs');
const path = require('path');
const { ApplePaths } = require('./lib/apple-paths');

const rootDir = path.resolve(__dirname, '../..');
const source = path.join(rootDir, 'client/src-tauri/Info.ios.plist');

if (!fs.existsSync(source)) {
  console.error(`Not found: ${source}`);
  process.exit(1);
}

const target = ApplePaths.infoPlist(rootDir);

fs.copyFileSync(source, target);

console.log(`Installed ${source}`);
console.log(`       to ${target}`);
