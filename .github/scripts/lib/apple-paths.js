/**
 * Locates the pieces of the generated Xcode project.
 *
 * `tauri ios init` names the target directory and the project after the Cargo
 * package, so the names are derivable but not stable: a rename turns a
 * hardcoded path into a CI failure that names a missing file rather than the
 * rename. Everything here is found, and every miss is loud.
 */
const fs = require('fs');
const path = require('path');

const APPLE_DIR = 'client/src-tauri/gen/apple';

class ApplePaths {
  static root(rootDir) {
    const apple = path.join(rootDir, APPLE_DIR);

    if (!fs.existsSync(apple)) {
      console.error(`Not found: ${apple}`);
      console.error('Run `yarn tauri ios init` before this script.');
      process.exit(1);
    }

    return apple;
  }

  static only(apple, suffix, what) {
    const found = fs.readdirSync(apple).filter((entry) => entry.endsWith(suffix));

    if (found.length !== 1) {
      console.error(`Expected exactly one ${what} in ${apple}, found ${found.length}.`);
      if (found.length > 1) console.error(`  ${found.join('\n  ')}`);
      process.exit(1);
    }

    return path.join(apple, found[0]);
  }

  /** The `<name>_iOS` directory holding Info.plist and the entitlements file. */
  static target(rootDir) {
    return ApplePaths.only(ApplePaths.root(rootDir), '_iOS', 'iOS target directory');
  }

  /** The generated Info.plist. */
  static infoPlist(rootDir) {
    return path.join(ApplePaths.target(rootDir), 'Info.plist');
  }

  /**
   * The generated entitlements file, which Tauri names after its directory.
   */
  static entitlements(rootDir) {
    const target = ApplePaths.target(rootDir);

    return path.join(target, `${path.basename(target)}.entitlements`);
  }

  /** The `project.pbxproj` inside the single generated .xcodeproj. */
  static pbxproj(rootDir) {
    const project = ApplePaths.only(ApplePaths.root(rootDir), '.xcodeproj', 'Xcode project');

    return path.join(project, 'project.pbxproj');
  }
}

module.exports = { ApplePaths };
