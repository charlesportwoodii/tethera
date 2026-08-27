use crate::display_name::DisplayName;
use crate::state::AppState;
use std::io::Read;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};
use tethera_client_core::rpc::Rpc;
use tethera_client_core::transfer::{Digest, Fetch, Put};
use tethera_common::protocol::response::Payload;
use tethera_common::protocol::transfer::{FetchSpec, PutSpec};
use tethera_common::protocol::Request;
use tethera_common::structs::client::{Attached, AssetPreview};
use tethera_common::structs::ids::{AssetId, ServerId};

/// Moving files between a phone and a machine.
///
/// Everything here runs in Rust, including the picker and the writing. No
/// `dialog:` or `fs:` permission is granted to the webview, for the reason the
/// keyring holds none: this app draws agent transcripts with `csp: null`, and a
/// webview that could call `fs:write-file` could write anywhere this process
/// reaches. What crosses the boundary is a name and a result.
pub struct Assets;

impl Assets {
    /// The most this will read into memory for one upload.
    ///
    /// A ceiling exists because the digest has to be computed over the whole
    /// file before the first byte is sent - the machine is told what is coming
    /// and checks it - so the file is held once. A phone photo is a few
    /// megabytes; this is the point past which a person should be told no rather
    /// than have the app killed by the system for growing too large.
    pub const MAX_UPLOAD: u64 = 64 * 1024 * 1024;

    /// The largest image this will carry into the webview to be shown.
    ///
    /// An image cannot be previewed a piece at a time - it has to arrive whole
    /// to decode - so unlike text there is no reading the head of one. Past this
    /// the reader is told to save it instead, which is the honest answer on a
    /// phone.
    pub const MAX_IMAGE_PREVIEW: u64 = 8 * 1024 * 1024;

    /// The channel a composer listens on while a file is going up.
    pub const UPLOAD_PROGRESS: &'static str = "upload-progress";

    /// The share of replacement characters past which a decode is not text.
    ///
    /// Generous, because a log with a handful of bad bytes is still worth
    /// reading. Binary decoded as UTF-8 lands far above this - a PNG is mostly
    /// replacements - so the two cases are not close together.
    const READABLE: f32 = 0.1;

    /// Whether a lossy decode produced something worth showing as text.
    fn readable(text: &str) -> bool {
        if text.is_empty() {
            return true;
        }

        let total = text.chars().count();
        let lost = text.chars().filter(|ch| *ch == char::REPLACEMENT_CHARACTER).count();

        (lost as f32 / total as f32) < Self::READABLE
    }

    fn asset(value: &str) -> Result<AssetId, String> {
        AssetId::parse(value).ok_or_else(|| format!("{value} is not an asset id"))
    }

    fn server(value: &str) -> Result<ServerId, String> {
        ServerId::parse(value).ok_or_else(|| format!("{value} is not a server id"))
    }

    /// What to call a file the person picked.
    ///
    /// On a phone the picker answers a `content://` URI rather than a path, so
    /// there is no filename to read off it directly. The name still matters:
    /// the machine stores the file under it and the agent is handed that path,
    /// and an agent given an extensionless blob treats it differently from a
    /// `.png`.
    ///
    /// A document id from the Downloads or Documents provider usually carries
    /// the original path inside it - `raw:/storage/emulated/0/Download/a.txt`,
    /// percent-encoded - so the last segment that looks like a filename is read
    /// out of it. A provider that answers only an opaque row id gives nothing
    /// to read, and that is when the fallback is used.
    fn name_of(source: &FilePath) -> String {
        if let Ok(path) = source.clone().into_path() {
            if let Some(name) = path.file_name() {
                let name = name.to_string_lossy();

                if !name.is_empty() {
                    return name.into_owned();
                }
            }
        }

        let raw = source.to_string();

        // Asked, not guessed. A document id is usually opaque - the Downloads
        // provider answers `.../document/19` - so there is no name in the URI to
        // read. The authority is the trap: `com.android.providers.downloads.
        // documents` has dots in it, so any heuristic looking for a filename
        // finds that instead and is confidently wrong.
        if let Some(name) = DisplayName::of(&raw) {
            return name;
        }

        // A `file://` URI does carry its name, so its last *path* segment is
        // worth one look. Never the authority, for the reason above.
        let path = raw.split_once("://").map(|(_, rest)| rest).unwrap_or(&raw);

        match path.split_once('/') {
            Some((_authority, tail)) => Self::percent_decode(tail)
                .rsplit('/')
                .find(|part| !part.is_empty() && part.contains('.'))
                .map(str::to_string)
                .unwrap_or_else(|| "attachment".to_string()),
            None => "attachment".to_string(),
        }
    }
}

/// Starts saving a file the agent pushed, wherever the person chooses to put it.
///
/// Answers the id of the download, or `None` when the save dialog was
/// dismissed - an ordinary outcome and not a failure. It does **not** wait for
/// the bytes. A four hundred megabyte file takes as long as it takes, and a
/// command that only answered at the end left a person with no progress, no way
/// to cancel, and no way to tell a file that is still arriving from one that
/// has arrived. Everything after this point is reported on
/// `Downloads::CHANNEL`, keyed by the id returned here.
#[tauri::command]
pub(crate) async fn download_asset(
    app: AppHandle,
    state: State<'_, AppState>,
    server: String,
    asset: String,
    name: String,
) -> Result<Option<String>, String> {
    let server = Assets::server(&server)?;
    let asset = Assets::asset(&asset)?;

    let (tell, hear) = tokio::sync::oneshot::channel();

    // Held across the dialog so backgrounding this app for its own save screen
    // does not read as somebody putting the phone down.
    let _holding = state.settings().holding();

    // The callback form rather than the blocking one: `blocking_save_file`
    // panics when it runs on the main thread, and which thread a command lands
    // on is not this code's to decide.
    app.dialog()
        .file()
        .set_file_name(&name)
        .save_file(move |chosen| {
            let _ = tell.send(chosen);
        });

    let Some(destination) = hear.await.map_err(|error| error.to_string())? else {
        return Ok(None);
    };

    Ok(Some(
        state
            .downloads()
            .start(app.clone(), server, asset, name, destination)
            .await,
    ))
}

/// Stops a download, keeping what has already arrived.
///
/// Silent when the id names nothing: a person tapping stop on a row that
/// finished a moment ago has done nothing wrong.
#[tauri::command]
pub(crate) async fn cancel_download(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    state.downloads().cancel(&id).await;

    Ok(())
}

/// Tells every paused download to ask again now.
///
/// Called when this app comes back to the front. A download interrupted by
/// somebody switching apps is asleep on a timer, and the moment worth acting on
/// is their return - not whenever that timer happens to expire.
#[tauri::command]
pub(crate) async fn resume_downloads(state: State<'_, AppState>) -> Result<(), String> {
    state.downloads().wake();

    Ok(())
}

/// Picks a file on this phone and stages it on the machine.
///
/// Staged, not delivered. The upload answers an id, and the id becomes real only
/// when a prompt is sent carrying it - so a file picked and never sent is an
/// orphan the machine clears up rather than something a person has to undo.
///
/// Answers `None` when the picker was dismissed.
#[tauri::command]
pub(crate) async fn attach_file(
    app: AppHandle,
    state: State<'_, AppState>,
    server: String,
) -> Result<Option<Attached>, String> {
    let server = Assets::server(&server)?;

    let (tell, hear) = tokio::sync::oneshot::channel();

    // Same reason as the save dialog: the picker is a separate activity, and
    // this app's own screens should not lock behind it.
    let _holding = state.settings().holding();

    app.dialog().file().pick_file(move |chosen| {
        let _ = tell.send(chosen);
    });

    let Some(source) = hear.await.map_err(|error| error.to_string())? else {
        return Ok(None);
    };

    let name = Assets::name_of(&source);

    // Announced before a byte is read. Everything below this - opening the
    // file, reading it whole, hashing it, asking the machine its ceiling - runs
    // before the first byte goes on the wire, and on a large file that is the
    // longest silent stretch of the whole operation. A chip at zero is the
    // difference between "this has started" and an app that appears to have
    // ignored the tap.
    let _ = app.emit(
        Assets::UPLOAD_PROGRESS,
        UploadProgress {
            name: name.clone(),
            sent: 0,
            total: 0,
        },
    );

    let mut file = app
        .fs()
        .open(source.clone(), OpenOptions::new().read(true).to_owned())
        .map_err(|error| format!("could not read {name}: {error}"))?;

    let mut body = Vec::new();
    file.read_to_end(&mut body)
        .map_err(|error| format!("could not read {name}: {error}"))?;

    let size = body.len() as u64;

    if size == 0 {
        return Err(format!("{name} is empty"));
    }

    if size > Assets::MAX_UPLOAD {
        return Err(format!(
            "{name} is {size} bytes; this app will not send more than {} in one attachment",
            Assets::MAX_UPLOAD
        ));
    }

    let connection = state.connect(&server).await?;

    // The machine's own ceiling, asked for rather than assumed. It is allowed to
    // be smaller than this app's, and refusing here says so plainly instead of
    // sending a file that is rejected after the whole transfer.
    if let Ok(Payload::Describe(describe)) = Rpc::request(&connection, Request::Describe).await {
        if let Some(bound) = describe.limits.max_upload {
            if size > bound {
                return Err(format!(
                    "{name} is {size} bytes and that machine accepts at most {bound}"
                ));
            }
        }
    }

    let sha256 = Digest::of(&body);

    let spec = PutSpec {
        name: name.clone(),
        len: size,
        sha256,
        offset: 0,
    };

    // Same retry as the download, for the same reason.
    let (ready, mut put) = match Put::open(&connection, spec.clone()).await {
        Ok(opened) => opened,
        Err(_) => {
            let connection = state.reconnect(&server).await?;

            Put::open(&connection, spec)
                .await
                .map_err(|error| error.to_string())?
        }
    };

    // The machine says where to start, which is believed over the zero that was
    // proposed. It may already hold part of this file from an attempt that was
    // cut off.
    let mut sent = usize::try_from(ready.offset).unwrap_or(0).min(body.len());

    while sent < body.len() {
        let end = (sent + Fetch::CHUNK).min(body.len());

        put.write(&body[sent..end])
            .await
            .map_err(|error| error.to_string())?;

        sent = end;

        // Ignored if nothing is listening. A screen that has moved on is not a
        // reason to fail a transfer that is working.
        let _ = app.emit(
            Assets::UPLOAD_PROGRESS,
            UploadProgress {
                name: name.clone(),
                sent: sent as u64,
                total: size,
            },
        );
    }

    let result = put.finish().await.map_err(|error| error.to_string())?;

    // The id the machine minted, recorded against the name it was minted for.
    // An attachment that reaches a prompt as a blank line is indistinguishable
    // from one that was never sent, and the only way to tell whether this end
    // asked for the wrong thing or that end lost it is to have both halves
    // written down.
    log::info!(
        "staged {} as {} ({} bytes)",
        name,
        result.asset.as_str(),
        size
    );

    Ok(Some(Attached {
        asset: result.asset,
        name,
        size,
    }))
}

impl Assets {
    /// Percent-decoding, without a dependency for eighteen lines of it.
    ///
    /// Lenient by design: a stray `%` that is not a valid escape is kept as
    /// itself rather than failing the whole name. This only ever produces a
    /// label for a file, so a malformed byte costs a worse name and never an
    /// error.
    fn percent_decode(raw: &str) -> String {
        let bytes = raw.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut index = 0;

        while index < bytes.len() {
            if bytes[index] == b'%' && index + 2 < bytes.len() {
                let pair = std::str::from_utf8(&bytes[index + 1..index + 3]).ok();

                if let Some(value) = pair.and_then(|pair| u8::from_str_radix(pair, 16).ok()) {
                    out.push(value);
                    index += 3;

                    continue;
                }
            }

            out.push(bytes[index]);
            index += 1;
        }

        String::from_utf8_lossy(&out).into_owned()
    }
}

/// How an upload is getting on, while it is getting on.
///
/// Emitted per chunk rather than at the end, because the end is the one moment
/// a person does not need telling: they can see the chip. What they need is the
/// minute before it, when a phone on a slow link looks like an app that has
/// stopped responding.
#[derive(Clone, serde::Serialize)]
pub(crate) struct UploadProgress {
    pub name: String,
    pub sent: u64,
    pub total: u64,
}

/// The head of a file, for showing it without saving it.
///
/// The stream is dropped once `limit` bytes have arrived, so a five gigabyte
/// dump costs a phone one chunk rather than five gigabytes. `truncated` says
/// that happened - without it a long file read short reads as a short file, and
/// somebody believes they have seen all of it.
#[tauri::command]
pub(crate) async fn preview_asset(
    state: State<'_, AppState>,
    server: String,
    asset: String,
    limit: u32,
    mime: Option<String>,
) -> Result<AssetPreview, String> {
    let server = Assets::server(&server)?;
    let asset = Assets::asset(&asset)?;
    let connection = state.connect(&server).await?;

    let (head, mut fetch) = match Fetch::open(
        &connection,
        FetchSpec {
            asset: asset.clone(),
            offset: 0,
        },
    )
    .await
    {
        Ok(opened) => opened,
        Err(_) => {
            let connection = state.reconnect(&server).await?;

            Fetch::open(&connection, FetchSpec { asset, offset: 0 })
                .await
                .map_err(|error| error.to_string())?
        }
    };

    let wanted = u64::from(limit);
    // The caller's mime is believed over the head's, because the head does not
    // carry one: the machine answers `None` for every asset, while the card in
    // the transcript knows the type from the record that produced it. Without
    // this a PNG is decoded as UTF-8 and the reader is shown its header bytes
    // as text, which is what happened.
    let kind = head.mime.clone().or(mime);

    let image = kind
        .as_deref()
        .map(|mime| mime.starts_with("image/") && !mime.contains("svg"))
        .unwrap_or(false);

    // An image needs every byte or it will not decode, so the ceiling is the
    // whole file rather than a window into it - and a file past the ceiling is
    // refused rather than fetched and then discarded.
    if image && head.len > Assets::MAX_IMAGE_PREVIEW {
        return Ok(AssetPreview {
            mime: kind,
            len: Some(head.len),
            text: None,
            image_data_url: None,
            truncated: true,
        });
    }

    let ceiling = if image { head.len } else { wanted };
    let mut body: Vec<u8> = Vec::new();

    while (body.len() as u64) < ceiling {
        match fetch.next().await.map_err(|error| error.to_string())? {
            Some(chunk) => body.extend_from_slice(&chunk),
            None => break,
        }
    }

    // Dropped deliberately rather than read to the end. Resetting the stream is
    // how the machine is told this reader has enough.
    let truncated = !fetch.complete();
    drop(fetch);

    if image {
        use base64::Engine;

        let mime = kind.clone().unwrap_or_else(|| "image/png".to_string());
        let encoded = base64::engine::general_purpose::STANDARD.encode(&body);

        return Ok(AssetPreview {
            mime: kind,
            len: Some(head.len),
            text: None,
            image_data_url: Some(format!("data:{mime};base64,{encoded}")),
            truncated,
        });
    }

    body.truncate(wanted as usize);

    let text = String::from_utf8_lossy(&body).into_owned();

    Ok(AssetPreview {
        mime: kind,
        len: Some(head.len),
        // Lossy, but only up to a point. A stray byte in a log costs one
        // replacement character and the file stays readable, which is what
        // lossy decoding is for. A file that is *mostly* replacement characters
        // is not text at all, and showing its bytes as glyphs tells the reader
        // their file is corrupt when it is a perfectly good PNG. Better to say
        // there is nothing to show.
        text: if Assets::readable(&text) { Some(text) } else { None },
        image_data_url: None,
        truncated,
    })
}
