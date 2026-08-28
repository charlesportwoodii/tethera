// Appended to the generated `gen/android/app/build.gradle.kts` after
// `tauri android init`, which rewrites that file on every run.
//
// A second top-level `android { }` block merges into the same extension rather
// than replacing it, and Kotlin DSL evaluates top-level statements in order, so
// the `buildTypes` clause below reconfigures the release type the template
// already declared.
//
// No `import` lines: those are only legal at the top of a `.kts` file, and this
// text arrives at the bottom of one. Every type here is fully qualified for
// that reason.
//
// Gradle signs both the APK and the AAB from this one config, and it reads the
// passwords out of a properties file. Signing the finished artifacts instead
// would mean `jarsigner -storepass <secret>`, and argv on a build runner is
// readable through `ps`.

val tetheraKeystoreProperties = java.util.Properties()
val tetheraKeystoreFile = rootProject.file("keystore.properties")

if (tetheraKeystoreFile.exists()) {
    tetheraKeystoreFile.inputStream().use { tetheraKeystoreProperties.load(it) }
}

android {
    signingConfigs {
        // maybeCreate rather than create: a future Tauri template that declares
        // its own `release` config would otherwise fail the build with
        // "SigningConfig with name 'release' already exists", which reads as a
        // duplicated block rather than as a template change.
        maybeCreate("release").apply {
            storeFile = file(tetheraKeystoreProperties.getProperty("storeFile"))
            storePassword = tetheraKeystoreProperties.getProperty("storePassword")
            keyAlias = tetheraKeystoreProperties.getProperty("keyAlias")
            keyPassword = tetheraKeystoreProperties.getProperty("keyPassword")
        }
    }

    buildTypes {
        getByName("release") {
            signingConfig = signingConfigs.getByName("release")
        }
    }
}
