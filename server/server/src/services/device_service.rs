use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    QueryFilter,
};
use std::str::FromStr;
use std::sync::Arc;
use tethera_common::structs::device::{Device, DeviceState};

pub struct DeviceService {
    db: Arc<DatabaseConnection>,
}

impl DeviceService {
    /// Long enough for "Charl's iPhone 17 Pro", short enough that the value
    /// cannot be used as storage.
    pub const MAX_NAME_CHARS: usize = 64;

    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    pub fn new_shared(db: Arc<DatabaseConnection>) -> Arc<Self> {
        Arc::new(Self::new(db))
    }

    pub async fn list<C: ConnectionTrait>(&self, conn: &C) -> anyhow::Result<Vec<Device>> {
        let rows = tethera_entity::device::Entity::find().all(conn).await?;

        rows.into_iter().map(Self::to_dto).collect()
    }

    /// The one device an operator meant, from what they were willing to type.
    ///
    /// An endpoint id is 64 hex characters, so requiring the whole thing would
    /// make every device command a copy-paste exercise. A unique prefix is
    /// accepted the way a short commit hash is — and an ambiguous one is
    /// refused with the candidates rather than resolved to the first match,
    /// because the commands built on this revoke and ban.
    pub async fn resolve<C: ConnectionTrait>(
        &self,
        conn: &C,
        needle: &str,
    ) -> anyhow::Result<Device> {
        if needle.is_empty() {
            anyhow::bail!("name a device by its endpoint id, or by enough of the front of one");
        }

        if let Some(exact) = self.find_by_endpoint(conn, needle).await? {
            return Ok(exact);
        }

        let candidates: Vec<Device> = self
            .list(conn)
            .await?
            .into_iter()
            .filter(|device| device.endpoint_id.starts_with(needle))
            .collect();

        match candidates.len() {
            0 => anyhow::bail!("no device here has an endpoint id starting with {needle}"),
            1 => Ok(candidates.into_iter().next().expect("one candidate")),
            _ => {
                let named = candidates
                    .iter()
                    .map(|device| format!("{} ({})", device.endpoint_id, device.name))
                    .collect::<Vec<_>>()
                    .join("\n  ");

                anyhow::bail!("{needle} names more than one device:\n  {named}")
            }
        }
    }

    pub async fn find_by_endpoint<C: ConnectionTrait>(
        &self,
        conn: &C,
        endpoint_id: &str,
    ) -> anyhow::Result<Option<Device>> {
        let row = Self::row_by_endpoint(conn, endpoint_id).await?;

        row.map(Self::to_dto).transpose()
    }

    /// Enrols this endpoint id, or re-enrols one that was already here.
    ///
    /// The transition is judged by the state machine like any other, so a
    /// revoked device cannot be returned to `Active` by a code that is still
    /// inside its window.
    pub async fn activate<C: ConnectionTrait>(
        &self,
        conn: &C,
        endpoint_id: &str,
        name: &str,
        now: i64,
    ) -> anyhow::Result<Device> {
        let name = &Self::sanitise_name(name);
        let existing = Self::row_by_endpoint(conn, endpoint_id).await?;

        let Some(row) = existing else {
            let inserted = tethera_entity::device::ActiveModel {
                endpoint_id: ActiveValue::Set(endpoint_id.to_string()),
                name: ActiveValue::Set(name.to_string()),
                state: ActiveValue::Set(DeviceState::Active.as_str().to_string()),
                paired_at: ActiveValue::Set(Some(now)),
                last_seen_at: ActiveValue::Set(Some(now)),
                ..Default::default()
            }
            .insert(conn)
            .await?;

            return Self::to_dto(inserted);
        };

        let current = DeviceState::from_str(&row.state)?;

        if !current.can_transition_to(DeviceState::Active) {
            anyhow::bail!("cannot enrol a device in state {}", current.as_str());
        }

        let mut active: tethera_entity::device::ActiveModel = row.into();
        active.name = ActiveValue::Set(name.to_string());
        active.state = ActiveValue::Set(DeviceState::Active.as_str().to_string());
        active.paired_at = ActiveValue::Set(Some(now));
        active.last_seen_at = ActiveValue::Set(Some(now));

        Self::to_dto(active.update(conn).await?)
    }

    pub async fn set_state<C: ConnectionTrait>(
        &self,
        conn: &C,
        endpoint_id: &str,
        next: DeviceState,
        now: i64,
    ) -> anyhow::Result<Device> {
        let row = Self::row_by_endpoint(conn, endpoint_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("no device is enrolled for that endpoint id"))?;

        let current = DeviceState::from_str(&row.state)?;

        // Already there. A retry after a dropped acknowledgement is the ordinary
        // case, and `can_transition_to` has no self-pair, so treating this as a
        // refusal would tell a device that revoked itself twice that it does not
        // exist.
        if current == next {
            return Self::to_dto(row);
        }

        if !current.can_transition_to(next) {
            anyhow::bail!(
                "a device in state {} cannot move to {}",
                current.as_str(),
                next.as_str()
            );
        }

        let mut active: tethera_entity::device::ActiveModel = row.into();
        active.state = ActiveValue::Set(next.as_str().to_string());
        active.last_seen_at = ActiveValue::Set(Some(now));

        Self::to_dto(active.update(conn).await?)
    }

    pub async fn touch<C: ConnectionTrait>(
        &self,
        conn: &C,
        endpoint_id: &str,
        now: i64,
    ) -> anyhow::Result<()> {
        let Some(row) = Self::row_by_endpoint(conn, endpoint_id).await? else {
            return Ok(());
        };

        let mut active: tethera_entity::device::ActiveModel = row.into();
        active.last_seen_at = ActiveValue::Set(Some(now));
        active.update(conn).await?;

        Ok(())
    }

    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    /// A device name arrives from an endpoint that is not yet enrolled, bounded
    /// only by the frame cap, and is later printed to the operator's own
    /// terminal by `device list`. Control characters in it are terminal
    /// injection into the machine's shell from an unauthenticated peer.
    fn sanitise_name(name: &str) -> String {
        let cleaned: String = name
            .chars()
            .filter(|c| !c.is_control() && !Self::is_invisible(*c))
            .take(Self::MAX_NAME_CHARS)
            .collect();

        let trimmed = cleaned.trim();

        if trimmed.is_empty() {
            return "unnamed device".to_string();
        }

        trimmed.to_string()
    }

    // Zero-width and bidirectional-override characters are not control
    // characters, and they let two devices render identically in `device list`.
    // A name that cannot be told apart from another is worse than a truncated
    // one.
    fn is_invisible(c: char) -> bool {
        matches!(
            c,
            '\u{200b}'..='\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{feff}'
        )
    }

    async fn row_by_endpoint<C: ConnectionTrait>(
        conn: &C,
        endpoint_id: &str,
    ) -> anyhow::Result<Option<tethera_entity::device::Model>> {
        Ok(tethera_entity::device::Entity::find()
            .filter(tethera_entity::device::Column::EndpointId.eq(endpoint_id))
            .one(conn)
            .await?)
    }

    // Entity models never leave a service. The DTO is the boundary.
    //
    // A row whose state this build does not recognise fails the read. Mapping
    // it to Pending would turn a banned device into an approvable one on the
    // strength of a typo.
    fn to_dto(model: tethera_entity::device::Model) -> anyhow::Result<Device> {
        Ok(Device {
            id: model.id.to_string(),
            name: model.name,
            endpoint_id: model.endpoint_id,
            state: DeviceState::from_str(&model.state)?,
            paired_at: model.paired_at,
            last_seen_at: model.last_seen_at,
        })
    }
}
