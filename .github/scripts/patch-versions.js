#!/usr/bin/env node
/**
 * Patches version numbers across every file in the workspace that records one.
 * Usage: node patch-versions.js <version>
 *
 * Files patched:
 * - Cargo.toml ([workspace.package] version, which every member inherits)
 * - Cargo.lock (the resolved version of each workspace member)
 * - client/src-tauri/tauri.conf.json (version, versionCode, bundleVersion)
 * - client/src-tauri/Info.ios.plist (CFBundleShortVersionString, CFBundleVersion)
 * - client/package.json
 *
 * semantic-release calls this from `prepareCmd`, so it runs before the release
 * commit is made and its output is part of that commit.
 */

const fs = require('fs');
const path = require('path');
const { VersionEncoder } = require('./lib/encode-version');

const version = process.argv[2];
if (!version) {
  console.error('Usage: node patch-versions.js <version>');
  process.exit(1);
}

/**
 * Patch Cargo.toml files - updates the version field
 */
function patchCargoToml(filePath, version) {
  if (!fs.existsSync(filePath)) {
    console.error(`File not found: ${filePath}`);
    process.exit(1);
  }
  const content = fs.readFileSync(filePath, 'utf8');
  const updated = content.replace(
    /^version\s*=\s*"[^"]*"/m,
    `version = "${version}"`
  );
  fs.writeFileSync(filePath, updated);
  console.log(`Patched: ${filePath}`);
}

/**
 * Patch tauri.conf.json - updates version, versionCode, and bundleVersion
 *
 * The `version` field is set to the encoded mod version (e.g. "1.0.508") instead
 * of the raw semver (e.g. "1.0.0-beta.8") because Tauri uses this field directly
 * for CFBundleShortVersionString on Apple platforms. Semver prerelease tags get
 * stripped and mangled by Tauri's xcode-script, so we must provide a clean
 * 3-component version.
 */
function patchTauriConf(filePath, version) {
  if (!fs.existsSync(filePath)) {
    console.error(`File not found: ${filePath}`);
    process.exit(1);
  }
  const content = JSON.parse(fs.readFileSync(filePath, 'utf8'));

  const encoded = VersionEncoder.encode(version);
  const displayVersion = `${encoded.major}.${encoded.minor}.${encoded.encodedPatch}`;

  content.version = displayVersion;

  if (!content.bundle) content.bundle = {};
  if (!content.bundle.android) content.bundle.android = {};
  content.bundle.android.versionCode = VersionEncoder.versionCode(version);

  const bundleVersion = String(VersionEncoder.versionCode(version));
  if (!content.bundle.iOS) content.bundle.iOS = {};
  content.bundle.iOS.bundleVersion = bundleVersion;
  if (!content.bundle.macOS) content.bundle.macOS = {};
  content.bundle.macOS.bundleVersion = bundleVersion;

  fs.writeFileSync(filePath, JSON.stringify(content, null, 2) + '\n');
  console.log(`Patched: ${filePath} (version: ${displayVersion}, versionCode: ${content.bundle.android.versionCode}, bundleVersion: ${bundleVersion})`);
}

/**
 * Patch package.json - updates version field
 */
function patchPackageJson(filePath, version) {
  if (!fs.existsSync(filePath)) {
    console.error(`File not found: ${filePath}`);
    process.exit(1);
  }
  const content = JSON.parse(fs.readFileSync(filePath, 'utf8'));
  content.version = version;
  fs.writeFileSync(filePath, JSON.stringify(content, null, 2) + '\n');
  console.log(`Patched: ${filePath}`);
}

/**
 * Patch Apple Info.plist - updates CFBundleShortVersionString and CFBundleVersion
 */
function patchInfoPlist(filePath, version) {
  if (!fs.existsSync(filePath)) {
    console.log(`Skipping (not found): ${filePath}`);
    return;
  }

  const encoded = VersionEncoder.encode(version);
  const shortVersion = `${encoded.major}.${encoded.minor}.${encoded.encodedPatch}`;
  const bundleVersion = String(VersionEncoder.versionCode(version));

  let content = fs.readFileSync(filePath, 'utf8');

  content = content.replace(
    /(<key>CFBundleShortVersionString<\/key>\s*<string>)[^<]*/,
    `$1${shortVersion}`
  );

  content = content.replace(
    /(<key>CFBundleVersion<\/key>\s*<string>)[^<]*/,
    `$1${bundleVersion}`
  );

  fs.writeFileSync(filePath, content);
  console.log(`Patched: ${filePath} (CFBundleShortVersionString: ${shortVersion}, CFBundleVersion: ${bundleVersion})`);
}

/**
 * Patch Cargo.lock - updates the version for a specific package
 */
function patchCargoLock(filePath, packageName, version) {
  if (!fs.existsSync(filePath)) {
    console.log(`Skipping (not found): ${filePath}`);
    return;
  }
  const content = fs.readFileSync(filePath, 'utf8');
  const pattern = new RegExp(
    `(\\[\\[package\\]\\]\\nname = "${packageName}"\\nversion = ")[^"]*"`,
  );
  const updated = content.replace(pattern, `$1${version}"`);
  fs.writeFileSync(filePath, updated);
  console.log(`Patched: ${filePath} (${packageName} -> ${version})`);
}

// Every workspace member, because Cargo.lock names each one separately.
// Listed rather than discovered: a member added without a line here leaves one
// stale version in the lock file, and the release still succeeds.
const WORKSPACE_MEMBERS = [
  'tethera-client',
  'tethera-client-core',
  'tethera-common',
  'tethera-entity',
  'tethera-migration',
  'tethera-relay',
  'tethera-server',
  'tethera-transport',
];

// Main execution
const rootDir = path.resolve(__dirname, '../..');

console.log(`Patching files to version ${version}...`);
console.log(`Android versionCode will be: ${VersionEncoder.versionCode(version)}`);
console.log('');

// One Cargo.toml, not one per crate. Every member spells its version
// `version.workspace = true`, so the root [workspace.package] entry is the only
// place the number lives — and the only place to patch it.
patchCargoToml(path.join(rootDir, 'Cargo.toml'), version);

// Cargo.lock still records a resolved version per member, and a lock file that
// disagrees with the manifest makes the next `cargo build` rewrite it. That
// rewrite lands as an uncommitted change on the release commit.
for (const member of WORKSPACE_MEMBERS) {
  patchCargoLock(path.join(rootDir, 'Cargo.lock'), member, version);
}

patchTauriConf(path.join(rootDir, 'client/src-tauri/tauri.conf.json'), version);
patchInfoPlist(path.join(rootDir, 'client/src-tauri/Info.ios.plist'), version);
patchPackageJson(path.join(rootDir, 'client/package.json'), version);

console.log('');
console.log(`All files patched to version ${version}`);
