use crate::config::ApplicationConfig;
use sea_orm::DatabaseConnection;
use sea_orm_migration::MigratorTrait;

pub struct Storage;

impl Storage {
    // Migrations run on every open. The database lives on the operator's own
    // machine with no separate deploy step, so nothing else would ever apply
    // them.
    pub async fn connect(config: &ApplicationConfig) -> anyhow::Result<DatabaseConnection> {
        config.ensure_data_dir()?;

        let connection = sea_orm::Database::connect(config.database_url()).await?;
        tethera_migration::Migrator::up(&connection, None).await?;

        Ok(connection)
    }
}
