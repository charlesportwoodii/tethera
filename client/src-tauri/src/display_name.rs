/// The name Android holds for a `content://` URI.
///
/// A document picker answers a URI, never a path, and the document id behind it
/// is usually opaque - `msf:1000000042` from the Downloads provider carries
/// nothing readable. Guessing from the URI text is worse than not trying: the
/// authority is `com.android.providers.downloads.documents`, which looks enough
/// like a filename to fool any heuristic based on dots.
///
/// The name is worth this much effort because it is not decoration. The machine
/// stores an upload under it and hands the agent that path, and an agent given
/// an extensionless blob treats it differently from a `.png`.
///
/// Everything here is Android-only and answers `None` everywhere else, which is
/// correct: no other platform hands back a URI in the first place.
pub struct DisplayName;

#[cfg(target_os = "android")]
impl DisplayName {
    /// `OpenableColumns.DISPLAY_NAME`. Named by string because the constant
    /// lives on a class this code has no binding for.
    const COLUMN: &'static str = "_display_name";

    pub fn of(uri: &str) -> Option<String> {
        match Self::query(uri) {
            Ok(name) => name,
            Err(error) => {
                // A name that cannot be read is not a failure worth stopping an
                // upload for. The caller falls back to a generic one.
                log::warn!("could not read a display name for {uri}: {error}");

                None
            }
        }
    }

    fn query(uri: &str) -> Result<Option<String>, jni::errors::Error> {
        use jni::objects::{JObject, JValue};

        let context = ndk_context::android_context();
        let vm = unsafe { jni::JavaVM::from_raw(context.vm().cast()) }?;
        let activity = unsafe { JObject::from_raw(context.context().cast()) };

        // Attached rather than assumed: this runs on a tokio worker, which the
        // JVM has never seen. `attach_current_thread` detaches on drop.
        let mut env = vm.attach_current_thread()?;

        let parsed = env.new_string(uri)?;
        let parsed = env
            .call_static_method(
                "android/net/Uri",
                "parse",
                "(Ljava/lang/String;)Landroid/net/Uri;",
                &[JValue::Object(&parsed.into())],
            )?
            .l()?;

        let resolver = env
            .call_method(
                &activity,
                "getContentResolver",
                "()Landroid/content/ContentResolver;",
                &[],
            )?
            .l()?;

        // A null projection asks for every column. Naming one risks a provider
        // that does not implement it throwing rather than answering nothing.
        let cursor = env
            .call_method(
                &resolver,
                "query",
                "(Landroid/net/Uri;[Ljava/lang/String;Landroid/os/Bundle;Landroid/os/CancellationSignal;)Landroid/database/Cursor;",
                &[
                    JValue::Object(&parsed),
                    JValue::Object(&JObject::null()),
                    JValue::Object(&JObject::null()),
                    JValue::Object(&JObject::null()),
                ],
            )?
            .l()?;

        if cursor.is_null() {
            return Ok(None);
        }

        let name = Self::read(&mut env, &cursor)?;

        // Closed on every path. A leaked cursor holds a provider connection open
        // for the life of the process.
        env.call_method(&cursor, "close", "()V", &[])?;

        Ok(name)
    }

    fn read(
        env: &mut jni::JNIEnv,
        cursor: &jni::objects::JObject,
    ) -> Result<Option<String>, jni::errors::Error> {
        use jni::objects::JValue;

        let column = env.new_string(Self::COLUMN)?;
        let index = env
            .call_method(
                cursor,
                "getColumnIndex",
                "(Ljava/lang/String;)I",
                &[JValue::Object(&column.into())],
            )?
            .i()?;

        if index < 0 {
            return Ok(None);
        }

        let moved = env.call_method(cursor, "moveToFirst", "()Z", &[])?.z()?;

        if !moved {
            return Ok(None);
        }

        let value = env
            .call_method(cursor, "getString", "(I)Ljava/lang/String;", &[JValue::Int(index)])?
            .l()?;

        if value.is_null() {
            return Ok(None);
        }

        let name: String = env.get_string(&value.into())?.into();

        Ok(if name.is_empty() { None } else { Some(name) })
    }
}

#[cfg(not(target_os = "android"))]
impl DisplayName {
    /// No other platform answers a URI from its picker, so there is nothing to
    /// look up.
    pub fn of(_uri: &str) -> Option<String> {
        None
    }
}
