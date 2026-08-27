fn main() {
    sync_android_main_activity();

    tauri_build::build()
}

/// Copies the tracked `android/MainActivity.kt` over the generated one.
///
/// `gen/android/` is produced by `tauri android init` and is gitignored, so a
/// customised activity placed there by hand is lost on the next regeneration and
/// absent in a fresh worktree. Keeping the real copy under `android/` and
/// writing it in from the build script is what makes it survive both.
///
/// The customisation is one JNI call. See the `initNdkContext` export in
/// `src/lib.rs` for why it is needed.
fn sync_android_main_activity() {
    let source = std::path::Path::new("android/MainActivity.kt");
    let target = std::path::Path::new(
        "gen/android/app/src/main/java/com/alaydriem/tethera/MainActivity.kt",
    );

    println!("cargo:rerun-if-changed={}", source.display());

    if !source.exists() {
        println!("cargo:warning=android/MainActivity.kt is missing; the ndk-context shim will not be compiled and an Android build will abort on launch");
        return;
    }

    let Some(parent) = target.parent() else {
        return;
    };

    // `tauri android init` has not run in this worktree. Nothing to overwrite,
    // and the file is written on the next build once the tree exists.
    if !parent.exists() {
        return;
    }

    let wanted = match std::fs::read(source) {
        Ok(bytes) => bytes,
        Err(error) => {
            println!("cargo:warning=could not read {}: {error}", source.display());
            return;
        }
    };

    // Written only when it differs, so the file's mtime does not change on every
    // build and force Gradle to recompile Kotlin each time.
    if std::fs::read(target).is_ok_and(|existing| existing == wanted) {
        return;
    }

    if let Err(error) = std::fs::write(target, &wanted) {
        println!("cargo:warning=could not write {}: {error}", target.display());
    }
}
