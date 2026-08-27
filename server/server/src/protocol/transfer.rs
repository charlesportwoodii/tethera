use crate::protocol::ports::{AssetPort, Ports};
use tethera_common::protocol::transfer::{FetchSpec, PutSpec};
use tethera_transport::error::TransportError;
use tethera_transport::frame::FrameCodec;
use tethera_transport::stream::FrameIo;

pub struct Transfer;

impl Transfer {
    /// How much of a file moves per read.
    ///
    /// Large enough that a big download is not a million syscalls, small enough
    /// that serving several at once does not add up to anything a machine would
    /// notice.
    const CHUNK_BYTES: usize = 64 * 1024;

    /// One framed head, then the raw bytes to FIN.
    ///
    /// Unframed on purpose: bulk transfer then pays no per-chunk overhead, and
    /// the control frame cap never constrains a file.
    pub async fn fetch<P: Ports>(
        ports: &P,
        codec: &FrameCodec,
        spec: FetchSpec,
        mut send: iroh::endpoint::SendStream,
    ) -> Result<(), TransportError> {
        let (head, mut body) = match ports.assets().fetch(&spec.asset, spec.offset).await {
            Ok(pair) => pair,
            // There is no error frame on a fetch stream, so a missing asset is
            // reported by resetting: the client sees a refusal rather than a
            // truncated file it might keep.
            Err(_) => {
                send.reset(1u32.into()).ok();

                return Ok(());
            }
        };

        let owed = head.len.saturating_sub(head.offset);

        FrameIo::write(&mut send, codec, &head).await?;

        // Read and written a chunk at a time, off the runtime's threads. A file
        // read whole into memory first bounds what this machine can serve by
        // what it can hold, which is a limit nobody chose.
        let mut chunk = vec![0u8; Self::CHUNK_BYTES];
        let mut served = 0u64;

        loop {
            let (read, filled) = tokio::task::spawn_blocking(move || {
                let read = std::io::Read::read(&mut body, &mut chunk);

                (read, (body, chunk))
            })
            .await
            .map_err(|error| TransportError::Connection(error.to_string()))?;

            (body, chunk) = filled;

            // A failed read is not an end of file, and the difference is the
            // whole of what this arm is for. Breaking here would finish the
            // stream cleanly after a partial body, and a truncated file reads
            // perfectly well — so it would reach the disk as a silent corruption
            // rather than as a message. Resetting makes it a refusal the client
            // can see.
            let read = match read {
                Ok(read) => read,
                Err(error) => {
                    tracing::warn!(
                        %error,
                        asset = spec.asset.as_str(),
                        served,
                        owed,
                        "a download stopped short: this machine could not read the rest"
                    );

                    send.reset(2u32.into()).ok();

                    return Ok(());
                }
            };

            if read == 0 {
                break;
            }

            // Ordinary on a phone rather than exceptional: locking the screen or
            // moving to another app suspends the connection mid-transfer. The
            // byte count is the point of the record — it is what says a resume
            // is worth offering, and from which offset.
            if let Err(error) = send.write_all(&chunk[..read]).await {
                tracing::info!(
                    %error,
                    asset = spec.asset.as_str(),
                    served,
                    owed,
                    "a download ended before the client had all of it"
                );

                return Ok(());
            }

            served += read as u64;
        }

        send.finish().ok();

        tracing::info!(
            asset = spec.asset.as_str(),
            served,
            offset = head.offset,
            "served a download whole"
        );

        Ok(())
    }

    /// One framed ready, then raw bytes, then one framed result.
    ///
    /// The ready frame is what makes an upload resumable: only the server knows
    /// how much of a previous attempt reached disk, so the client seeks to the
    /// offset named here rather than the one it proposed.
    pub async fn put<P: Ports>(
        ports: &P,
        codec: &FrameCodec,
        spec: PutSpec,
        mut send: iroh::endpoint::SendStream,
        mut recv: iroh::endpoint::RecvStream,
    ) -> Result<(), TransportError> {
        let ready = match ports.assets().put_ready(&spec).await {
            Ok(ready) => ready,
            Err(_) => {
                send.reset(1u32.into()).ok();

                return Ok(());
            }
        };

        FrameIo::write(&mut send, codec, &ready).await?;

        // Bounded by what the spec declared, not by when the peer decides to
        // stop. An unbounded read from an authenticated but untrusted peer is a
        // memory exhaustion the protocol can trivially avoid.
        let expected = spec.len.saturating_sub(ready.offset);
        let body = recv
            .read_to_end(expected as usize)
            .await
            .map_err(|error| TransportError::Connection(error.to_string()))?;

        let result = match ports.assets().put_finish(&spec, &body).await {
            Ok(result) => result,
            Err(_) => {
                send.reset(1u32.into()).ok();

                return Ok(());
            }
        };

        FrameIo::write(&mut send, codec, &result).await?;
        send.finish().ok();

        Ok(())
    }
}
