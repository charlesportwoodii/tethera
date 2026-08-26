use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tethera_common::errors::TetheraError;
use tethera_common::structs::device::DeviceState;
use tethera_common::structs::pairing::PairingCode;

pub struct PairingService {
    db: Arc<DatabaseConnection>,
}

impl PairingService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
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

    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }
}
