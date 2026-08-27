pub use sea_orm_migration::prelude::*;

mod m20260825_000001_initial;
mod m20260826_000002_pairing_attempts;

pub struct Migrator;

impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260825_000001_initial::Migration),
            Box::new(m20260826_000002_pairing_attempts::Migration),
        ]
    }
}
