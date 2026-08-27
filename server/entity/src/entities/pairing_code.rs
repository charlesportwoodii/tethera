use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "pairing_code")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub code_hash: String,
    pub expires_at: i64,
    pub consumed_at: Option<i64>,
    // The window's own budget, so redialling cannot reset it. The dispatcher
    // retries per connection, and a counter that lived there would make
    // attempts_left a number the server says rather than one it enforces.
    pub attempts_left: i32,
    pub code_length: i32,
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
