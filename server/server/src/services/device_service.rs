use sea_orm::{ConnectionTrait, DatabaseConnection, EntityTrait};
use std::str::FromStr;
use std::sync::Arc;
use tethera_common::structs::device::{Device, DeviceState};

pub struct DeviceService {
    db: Arc<DatabaseConnection>,
}

impl DeviceService {
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

    pub fn db(&self) -> &DatabaseConnection {
        &self.db
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
