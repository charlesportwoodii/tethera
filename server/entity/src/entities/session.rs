use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "session")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub session_key: String,
    pub workspace_id: String,
    pub agent: String,
    pub cwd: String,
    pub started_at: i64,
    pub last_active_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C: ConnectionTrait>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr> {
        let now = chrono::Utc::now().timestamp();

        if insert {
            self.created_at = sea_orm::ActiveValue::Set(now);
        }

        self.updated_at = sea_orm::ActiveValue::Set(now);

        Ok(self)
    }
}
