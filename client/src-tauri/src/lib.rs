mod commands;
mod display_name;
mod downloads;
mod keyring;
mod logging;
mod panes;
mod state;
mod machine_watch;
mod watches;

use downloads::Downloads;
use keyring::KeyringStore;
use logging::Logging;
use state::AppState;
use tauri::Manager;
use tethera_client_core::book::ServerBook;
use tethera_client_core::settings::SettingsStore;
use tethera_client_core::endpoint::ClientEndpoint;
use tethera_client_core::identity::Identity;
use tethera_common::protocol::handshake::ClientInfo;
use tethera_transport::endpoint::EndpointConfig;

/// Populates the global `ndk_context` static from the activity.
///
/// tao 0.35, which Tauri 2.11 depends on, stopped doing this as part of its
/// multi-activity refactor (tauri-apps/tao#1154). Nothing else does it, so
/// `ndk_context::android_context()` panics and aborts the process on first use.
/// Two things this app depends on read it: the keyring plugin's Android store
/// (open-source-cooperative/android-native-keyring-store#21) and iroh, through
/// `netdev` and `hickory-resolver`.
///
/// The symbol name is matched by the JVM against the declaring class, so it
/// encodes the package: renaming the package or the activity silently unbinds
/// this and the crash returns.
///
/// The context is kept as a `GlobalRef` for the life of the process, because the
/// local reference the JVM hands in here is invalid the moment this returns and
/// `ndk_context` holds the raw pointer indefinitely.
#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_alaydriem_tethera_MainActivity_initNdkContext(
    env: jni::JNIEnv,
    _class: jni::objects::JObject,
    context: jni::objects::JObject,
) {
    use jni::objects::GlobalRef;
    use std::ffi::c_void;
    use std::sync::OnceLock;

    static REF: OnceLock<Option<GlobalRef>> = OnceLock::new();

    REF.get_or_init(|| match env.new_global_ref(&context) {
        Ok(reference) => {
            let vm = env.get_java_vm().expect("a java vm from a live JNIEnv");
            let vm = vm.get_java_vm_pointer() as *mut c_void;

            unsafe {
                ndk_context::initialize_android_context(vm, reference.as_obj().as_raw() as _);
            }

            Some(reference)
        }
        Err(error) => {
            log::error!("could not hold the android context: {error}");

            None
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // reqwest 0.13, which iroh brings in for discovery, is built here with
    // `rustls-no-provider`. It does not pick a provider, it panics when a
    // Client is built without one - inside iroh, on a background thread, which
    // aborts the process rather than failing a dial. The abort happens the
    // first time anything is dialled, so it looks like a networking fault
    // rather than a missing one-line install.
    //
    // `ring` and not the crate's suggested aws-lc-rs, because the workspace
    // pins rustls to ring on purpose and a process holding two providers panics
    // in a different place. An error here means something already installed
    // one, which is the outcome this is asking for.
    let _ = rustls::crypto::ring::default_provider().install_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_curia::init())
        .plugin(tauri_plugin_keyring::init())
        .plugin(tauri_plugin_deep_link::init())
        // Neither of these grants the webview anything. Both are driven from
        // Rust only, and no `dialog:` or `fs:` permission appears in any
        // capability file, for the same reason the keyring holds none: this app
        // renders agent transcripts and terminal scrollback with `csp: null`,
        // which is the highest-value XSS target it will ever have. A webview
        // that could call `fs:write-file` directly could write anywhere the
        // process can reach. Picking a file and moving its bytes happens behind
        // a command that takes a name and returns a result.
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            #[cfg(mobile)]
            app.handle()
                .plugin(tauri_plugin_barcode_scanner::init())?;

            // Mobile only, because no desktop platform has a sensor. The lock
            // this drives is a launch gate and nothing more: it stops somebody
            // holding an unlocked phone, and the key in the keyring stays
            // readable to anything running in this process. Said plainly here
            // because the settings copy says it too, and a lock somebody
            // overestimates is worse than one they understand.
            #[cfg(mobile)]
            app.handle().plugin(tauri_plugin_biometric::init())?;

            // First, so everything below it is logged rather than lost.
            if let Err(error) = Logging::install(app.handle()) {
                eprintln!("could not install logging: {error}");
            }

            let store = KeyringStore::new(app.handle().clone());
            store.initialise()?;

            // The endpoint id derived from this key is what every paired machine
            // holds in its allow-list, so a failure here must stop the app rather
            // than quietly mint a second identity.
            let secret_key = Identity::load_or_create(&store)?;

            let data_dir = app.path().app_local_data_dir()?;
            let book = ServerBook::open(data_dir.join(ServerBook::FILE_NAME))?;
            let settings = SettingsStore::open(data_dir.join(SettingsStore::FILE_NAME));

            let client = ClientInfo {
                app_version: env!("CARGO_PKG_VERSION").to_string(),
                platform: AppState::platform(),
                install_id: book.install_id(),
            };

            let endpoint = tauri::async_runtime::block_on(ClientEndpoint::bind(
                EndpointConfig::new(secret_key),
            ))?;

            app.manage(AppState::new(
                endpoint,
                book,
                client,
                settings,
                Downloads::new(data_dir),
            ));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_version,
            commands::servers::list_servers,
            commands::servers::sweep_servers,
            commands::servers::forget_server,
            commands::servers::list_conversations,
            commands::sessions::list_agent_profiles,
            commands::sessions::can_start_sessions,
            commands::sessions::recent_cwds,
            commands::sessions::preview_conversation,
            commands::sessions::start_conversation,
            commands::conversation::conversation_transcript,
            commands::conversation::conversation_controls,
            commands::conversation::get_conversation,
            commands::conversation::watch_conversation,
            commands::conversation::unwatch_conversation,
            commands::conversation::resume_conversation,
            commands::conversation::send_prompt,
            commands::conversation::answer_question,
            commands::conversation::interrupt_conversation,
            commands::assets::download_asset,
            commands::assets::cancel_download,
            commands::assets::resume_downloads,
            commands::assets::attach_file,
            commands::assets::preview_asset,
            commands::settings::read_preferences,
            commands::settings::set_biometric_lock,
            commands::settings::is_unlocked,
            commands::settings::unlock,
            commands::settings::lock,
            commands::pairing::pair_begin,
            commands::pairing::pair_submit,
            commands::pairing::pair_cancel,
            commands::terminal::list_workspaces,
            commands::terminal::list_tabs,
            commands::terminal::list_panes,
            commands::terminal::pane_layout,
            commands::terminal::focus_tab,
            commands::terminal::watch_machine,
            commands::terminal::unwatch_machine,
            commands::terminal::terminal_controls,
            commands::terminal::attach_pane,
            commands::terminal::detach_pane,
            commands::terminal::pane_key,
            commands::terminal::pane_text,
            commands::terminal::open_terminal,
            commands::terminal::split_pane,
            commands::terminal::close_pane,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tethera");
}
