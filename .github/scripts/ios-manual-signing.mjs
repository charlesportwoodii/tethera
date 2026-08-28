// Rewrites the generated Xcode project to sign manually against a specific
// provisioning profile.
//
// `tauri ios init` writes a project set to Automatic signing, which asks Xcode
// to fetch a profile from a signed-in Apple ID. A runner has no Apple ID
// session — it has a distribution certificate in a temporary keychain and one
// `.mobileprovision` on disk — so automatic signing fails with "No profiles for
// '<bundle id>' were found", which reads as a missing profile rather than as the
// wrong signing style.
//
// Node rather than sed: this edits a structured file in several places and has
// to fail loudly when it matched nothing. A `sed` that matches nothing exits 0
// and the build carries on to an unsigned export.
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("../../", import.meta.url)));

class ManualSigning {
  static IDENTITY = "Apple Distribution";

  static required(name) {
    const value = process.env[name];

    if (!value) {
      console.error(`FAIL ${name} is not set. It is needed to sign the app.`);
      process.exit(1);
    }

    return value;
  }

  // Found rather than named. Tauri derives the project's name from
  // productName, so hard-coding it turns a rename into a CI failure that names
  // a missing file instead of the rename.
  static project() {
    const apple = resolve(root, "client/src-tauri/gen/apple");

    if (!existsSync(apple)) {
      console.error(`FAIL ${apple} does not exist. Run \`yarn tauri ios init\` first.`);
      process.exit(1);
    }

    const projects = readdirSync(apple).filter((entry) => entry.endsWith(".xcodeproj"));

    if (projects.length !== 1) {
      console.error(
        `FAIL expected exactly one .xcodeproj under ${apple}, found ${projects.length}.`,
      );
      process.exit(1);
    }

    return join(apple, projects[0], "project.pbxproj");
  }

  // Every buildSettings block, not just the app target's. The generated project
  // carries settings on the project, on the app target and on each Rust static
  // library target, and a block left on Automatic is enough to fail the export.
  static settings(block, teamId, profileUuid) {
    let out = block;

    const ensure = (key, line) => {
      if (!new RegExp(`${key}\\b`).test(out)) out += `\t\t\t\t${line}\n`;
    };

    ensure("CODE_SIGN_STYLE", "CODE_SIGN_STYLE = Manual;");
    ensure("DEVELOPMENT_TEAM", `DEVELOPMENT_TEAM = "${teamId}";`);
    ensure("CODE_SIGN_IDENTITY", `CODE_SIGN_IDENTITY = "${ManualSigning.IDENTITY}";`);
    ensure("PROVISIONING_PROFILE_SPECIFIER", 'PROVISIONING_PROFILE_SPECIFIER = "";');
    ensure("PROVISIONING_PROFILE", `PROVISIONING_PROFILE = "${profileUuid}";`);

    out = out
      .replace(/CODE_SIGN_STYLE = [^;]*;/g, "CODE_SIGN_STYLE = Manual;")
      .replace(/DEVELOPMENT_TEAM = "[^"]*"/g, `DEVELOPMENT_TEAM = "${teamId}"`)
      .replace(
        /CODE_SIGN_IDENTITY = "[^"]*"/g,
        `CODE_SIGN_IDENTITY = "${ManualSigning.IDENTITY}"`,
      )
      .replace(/PROVISIONING_PROFILE_SPECIFIER = "[^"]*"/g, 'PROVISIONING_PROFILE_SPECIFIER = ""')
      .replace(/PROVISIONING_PROFILE = "[^"]*"/g, `PROVISIONING_PROFILE = "${profileUuid}"`);

    return out;
  }

  static run() {
    const teamId = ManualSigning.required("APPLE_TEAM_ID");
    const profileUuid = ManualSigning.required("IOS_PROFILE_UUID");
    const path = ManualSigning.project();

    let pbx = readFileSync(path, "utf8");
    let blocks = 0;

    pbx = pbx.replace(/ProvisioningStyle = Automatic;/g, "ProvisioningStyle = Manual;");
    pbx = pbx.replace(/buildSettings = \{([^}]*)\}/g, (_match, inner) => {
      blocks += 1;

      return `buildSettings = {${ManualSigning.settings(inner, teamId, profileUuid)}}`;
    });

    if (blocks === 0) {
      console.error(`FAIL no buildSettings block was found in ${path}.`);
      console.error("      The project layout changed; this rewrite no longer applies.");
      process.exit(1);
    }

    if (/ProvisioningStyle = Automatic;/.test(pbx)) {
      console.error("FAIL an Automatic ProvisioningStyle survived the rewrite.");
      process.exit(1);
    }

    writeFileSync(path, pbx);

    console.log(`ok manual signing set on ${blocks} buildSettings blocks in ${path}`);
    console.log(`   team ${teamId}, identity ${ManualSigning.IDENTITY}, profile ${profileUuid}`);
  }
}

ManualSigning.run();
