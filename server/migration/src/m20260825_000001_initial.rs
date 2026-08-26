use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// MigrationTrait is decorated with #[async_trait::async_trait] in
// sea-orm-migration 2.0.2, so the impl needs the same attribute.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Device::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Device::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Device::EndpointId)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Device::Name).string().not_null())
                    .col(ColumnDef::new(Device::State).string().not_null())
                    .col(ColumnDef::new(Device::PairedAt).big_integer().null())
                    .col(ColumnDef::new(Device::LastSeenAt).big_integer().null())
                    .col(ColumnDef::new(Device::CreatedAt).big_integer().not_null())
                    .col(ColumnDef::new(Device::UpdatedAt).big_integer().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(PairingCode::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PairingCode::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PairingCode::CodeHash).string().not_null())
                    .col(
                        ColumnDef::new(PairingCode::ExpiresAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PairingCode::ConsumedAt).big_integer().null())
                    .col(
                        ColumnDef::new(PairingCode::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PairingCode::UpdatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Session::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Session::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Session::SessionKey)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Session::WorkspaceId).string().not_null())
                    .col(ColumnDef::new(Session::Agent).string().not_null())
                    .col(ColumnDef::new(Session::Cwd).string().not_null())
                    .col(ColumnDef::new(Session::StartedAt).big_integer().not_null())
                    .col(
                        ColumnDef::new(Session::LastActiveAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Session::CreatedAt).big_integer().not_null())
                    .col(ColumnDef::new(Session::UpdatedAt).big_integer().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Session::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(PairingCode::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Device::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Device {
    Table,
    Id,
    EndpointId,
    Name,
    State,
    PairedAt,
    LastSeenAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum PairingCode {
    Table,
    Id,
    CodeHash,
    ExpiresAt,
    ConsumedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Session {
    Table,
    Id,
    SessionKey,
    WorkspaceId,
    Agent,
    Cwd,
    StartedAt,
    LastActiveAt,
    CreatedAt,
    UpdatedAt,
}
