use crate::services::DeviceService;
use rand::Rng;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder, TransactionTrait,
};
use std::sync::Arc;
use tethera_common::errors::TetheraError;
use tethera_common::protocol::handshake::Handshake;
use tethera_common::structs::device::{Device, DeviceState};
use tethera_common::structs::pairing::PairingCode;

pub struct PairingService {
    db: Arc<DatabaseConnection>,
    devices: Arc<DeviceService>,
    // An optimisation, not the guarantee. Single use is enforced by the
    // conditional consume in `redeem_locked`, which is what makes it true
    // against another process running `pair` on the same file. This only keeps
    // two tasks in this process from racing into that statement and one of them
    // losing on a database lock.
    redemption: tokio::sync::Mutex<()>,
}

impl PairingService {
    /// Six digits is 10^6 values. Five attempts inside a five-minute window is
    /// five in a million, and a person mistyping once is the common case rather
    /// than the exceptional one.
    pub const DEFAULT_ATTEMPTS: i32 = 5;
    pub const CODE_DIGITS: i32 = 6;
    pub const DEFAULT_TTL_SECONDS: u64 = 300;

    /// A day. Beyond this the value is a mistake rather than a choice, and a
    /// window that stays open for a year is not a window.
    pub const MAX_TTL_SECONDS: u64 = 86_400;

    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            devices: DeviceService::new_shared(db.clone()),
            db,
            redemption: tokio::sync::Mutex::new(()),
        }
    }

    pub fn new_shared(db: Arc<DatabaseConnection>) -> Arc<Self> {
        Arc::new(Self::new(db))
    }

    // All four checks are required and none is redundant. Expiry and
    // consumption come first so a dead code is never compared at all, and the
    // state check stops a revoked device being re-approved by replaying a code
    // that is still inside its window, which would make revocation advisory.
    //
    // consumed_at is a parameter rather than a lookup because a code that
    // cannot be observed as spent is single-use in name only: one code would
    // approve unlimited devices for the whole of its window.
    pub fn approve(
        current: DeviceState,
        stored: &PairingCode,
        supplied: &str,
        expires_at: i64,
        consumed_at: Option<i64>,
        now: i64,
    ) -> Result<DeviceState, TetheraError> {
        if now >= expires_at {
            return Err(TetheraError::PairingCodeExpired);
        }

        if consumed_at.is_some() {
            return Err(TetheraError::PairingCodeAlreadyUsed);
        }

        if !stored.verify(supplied) {
            return Err(TetheraError::PairingCodeMismatch);
        }

        if !current.can_transition_to(DeviceState::Active) {
            return Err(TetheraError::DeviceNotFound(format!(
                "cannot approve a device in state {current:?}"
            )));
        }

        Ok(DeviceState::Active)
    }

    /// Opens a pairing window, and returns the plaintext exactly once.
    ///
    /// The window is the row: an unconsumed, unexpired code with attempts left.
    /// Nothing else holds it, so it survives a restart of the server and needs
    /// no channel between this command and the running process.
    ///
    /// Minting supersedes every earlier open window. One window at a time is a
    /// far smaller thing to reason about than a set of them, and a code nobody
    /// remembers opening is a guessing surface. The count of windows it closed
    /// is returned so the command can say so: silently invalidating a code that
    /// is on screen costs somebody a confusing five minutes.
    pub async fn open_window(
        &self,
        ttl_seconds: u64,
        now: i64,
    ) -> anyhow::Result<(String, usize)> {
        // Held for the same reason redemption holds it: supersede-then-insert is
        // a read-then-write, and two of them interleaved leave two open rows
        // where `current_window` promises one.
        let _guard = self.redemption.lock().await;

        if ttl_seconds == 0 || ttl_seconds > Self::MAX_TTL_SECONDS {
            anyhow::bail!(
                "a pairing window must last between 1 and {} seconds",
                Self::MAX_TTL_SECONDS
            );
        }

        let plaintext = Self::generate();
        let transaction = self.db.begin().await?;

        let open = Self::open_rows(&transaction).await?;
        let superseded_count = open.len();

        for row in open {
            let mut superseded: tethera_entity::pairing_code::ActiveModel = row.into();
            superseded.consumed_at = ActiveValue::Set(Some(now));
            superseded.update(&transaction).await?;
        }

        // Hashed in the form the comparison will use. The dispatcher normalises
        // what a person typed before it reaches redemption, so storing the raw
        // form would fail the day a code is not all digits.
        let digest = PairingCode::from_plaintext(&Handshake::normalize_code(&plaintext)).to_hex();

        tethera_entity::pairing_code::ActiveModel {
            code_hash: ActiveValue::Set(digest),
            expires_at: ActiveValue::Set(now + ttl_seconds as i64),
            consumed_at: ActiveValue::Set(None),
            attempts_left: ActiveValue::Set(Self::DEFAULT_ATTEMPTS),
            code_length: ActiveValue::Set(Self::CODE_DIGITS),
            ..Default::default()
        }
        .insert(&transaction)
        .await?;

        transaction.commit().await?;

        Ok((plaintext, superseded_count))
    }

    /// The open window, if a human has opened one.
    pub async fn current_window<C: ConnectionTrait>(
        &self,
        conn: &C,
        now: i64,
    ) -> anyhow::Result<Option<tethera_entity::pairing_code::Model>> {
        Ok(tethera_entity::pairing_code::Entity::find()
            .filter(tethera_entity::pairing_code::Column::ConsumedAt.is_null())
            .filter(tethera_entity::pairing_code::Column::ExpiresAt.gt(now))
            .filter(tethera_entity::pairing_code::Column::AttemptsLeft.gt(0))
            .order_by_desc(tethera_entity::pairing_code::Column::Id)
            .one(conn)
            .await?)
    }

    /// Compares a typed code and enrols on success. The error is the number of
    /// attempts the window has left.
    ///
    /// Consuming the code and moving the device to `Active` happen in one
    /// transaction, and the consume is conditional on the code still being
    /// unconsumed. Both halves are needed: without the transaction a crash
    /// between them leaves a spent code and an unenrolled device, and without
    /// the condition two readers that both saw `consumed_at = None` would both
    /// be approved, which is single use defeated with nothing going red.
    pub async fn redeem(
        &self,
        endpoint_id: &str,
        code: &str,
        device_name: &str,
        now: i64,
    ) -> Result<Device, u8> {
        let _guard = self.redemption.lock().await;

        // Normalised here as well as in the dispatcher. `open_window` hashes the
        // normalised form, so a caller that skipped this would compare the wrong
        // thing - and the day the alphabet stops being digits, that stops being
        // harmless. Doing it twice costs a trim and an uppercase.
        let code = Handshake::normalize_code(code);

        match self
            .redeem_locked(endpoint_id, &code, device_name, now)
            .await
        {
            Ok(outcome) => outcome,
            // The port reports attempts remaining and has no way to say
            // "storage failed", so the honest answer is that this window has
            // nothing left to give and the reason goes to the log.
            Err(error) => {
                tracing::error!(%error, "pairing redemption failed");

                Err(0)
            }
        }
    }

    pub fn devices(&self) -> &Arc<DeviceService> {
        &self.devices
    }

    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    async fn redeem_locked(
        &self,
        endpoint_id: &str,
        code: &str,
        device_name: &str,
        now: i64,
    ) -> anyhow::Result<Result<Device, u8>> {
        let transaction = self.db.begin().await?;

        let Some(row) = self.current_window(&transaction, now).await? else {
            transaction.rollback().await?;

            // No window is open, so nothing remains to be spent against one.
            // The dispatcher closes the stream rather than inviting a guess it
            // could never satisfy.
            return Ok(Err(0));
        };

        let stored = PairingCode::from_hex(&row.code_hash)?;
        let current = self
            .devices
            .find_by_endpoint(&transaction, endpoint_id)
            .await?
            .map(|device| device.state)
            // A stranger is a device that has not been approved, which is what
            // Pending means. No row is written for a failed attempt, so
            // guessing cannot fill the device table.
            .unwrap_or(DeviceState::Pending);

        let attempts_left = row.attempts_left;
        let verdict = Self::approve(
            current,
            &stored,
            code,
            row.expires_at,
            row.consumed_at,
            now,
        );

        let window_id = row.id;

        if let Err(refusal) = verdict {
            // `approve` compares the digest before it looks at the device's
            // state, so a state refusal means the code was *right*. Charging the
            // window's budget there would let a device that can never enrol
            // spend the operator's five guesses. Only a mismatch is a guess.
            let mismatched = matches!(refusal, TetheraError::PairingCodeMismatch);
            let remaining = if mismatched {
                (attempts_left - 1).max(0)
            } else {
                attempts_left
            };

            if mismatched {
                let mut window: tethera_entity::pairing_code::ActiveModel = row.into();
                window.attempts_left = ActiveValue::Set(remaining);
                window.update(&transaction).await?;
            }

            transaction.commit().await?;

            return Ok(Err(Self::narrow(remaining)));
        }

        // Conditional, and the row count is the answer. An unconditional
        // `UPDATE ... WHERE id = ?` would be correct SQL for two transactions
        // that both read an unconsumed code, and single use would then rest on
        // whatever locking the database happens to do rather than on anything
        // stated here.
        let consumed = tethera_entity::pairing_code::Entity::update_many()
            .col_expr(
                tethera_entity::pairing_code::Column::ConsumedAt,
                Expr::value(now),
            )
            .col_expr(
                tethera_entity::pairing_code::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(tethera_entity::pairing_code::Column::Id.eq(window_id))
            .filter(tethera_entity::pairing_code::Column::ConsumedAt.is_null())
            .exec(&transaction)
            .await?;

        if consumed.rows_affected == 0 {
            transaction.rollback().await?;

            // Somebody else consumed this code between the read and the write.
            // Nothing was spent, and the window is gone.
            return Ok(Err(0));
        }

        let device = self
            .devices
            .activate(&transaction, endpoint_id, device_name, now)
            .await?;

        transaction.commit().await?;

        Ok(Ok(device))
    }

    async fn open_rows<C: ConnectionTrait>(
        conn: &C,
    ) -> anyhow::Result<Vec<tethera_entity::pairing_code::Model>> {
        Ok(tethera_entity::pairing_code::Entity::find()
            .filter(tethera_entity::pairing_code::Column::ConsumedAt.is_null())
            .all(conn)
            .await?)
    }

    pub fn narrow(attempts_left: i32) -> u8 {
        attempts_left.clamp(0, i32::from(u8::MAX)) as u8
    }

    // Derived from CODE_DIGITS rather than written twice, so changing the
    // constant changes the code rather than silently doing nothing.
    fn generate() -> String {
        let ceiling = 10u32.pow(Self::CODE_DIGITS as u32);
        let width = Self::CODE_DIGITS as usize;

        format!(
            "{:0width$}",
            rand::thread_rng().gen_range(0..ceiling),
            width = width
        )
    }
}
