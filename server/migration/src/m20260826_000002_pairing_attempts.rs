use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// MigrationTrait is decorated with #[async_trait::async_trait] in
// sea-orm-migration 2.0.2, so the impl needs the same attribute.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    // Two statements rather than one: SQLite adds a single column per
    // ALTER TABLE, and a constant default is the only default it accepts there.
    // Not `add_column_if_not_exists`: SQLite has no such form, and the sea-query
    // SQLite builder discards the flag, so the name would promise a safety the
    // statement does not have.
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(PairingCode::Table)
                    .add_column(
                        ColumnDef::new(PairingCode::AttemptsLeft)
                            .integer()
                            .not_null()
                            .default(5),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(PairingCode::Table)
                    .add_column(
                        ColumnDef::new(PairingCode::CodeLength)
                            .integer()
                            .not_null()
                            .default(6),
                    )
                    .to_owned(),
            )
            .await?;

        // Every row that predates this migration predates the concept of a
        // window, so none of them may be one. Without this, an unconsumed code
        // left by an older `pair code` comes out of the migration with a full
        // attempt budget and satisfies every filter `current_window` applies —
        // the machine would open a window nobody opened, with a code nobody
        // remembers, and hand it to the next endpoint that asks to enrol.
        manager
            .exec_stmt(
                Query::update()
                    .table(PairingCode::Table)
                    .value(PairingCode::AttemptsLeft, 0)
                    .and_where(Expr::col(PairingCode::ConsumedAt).is_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(PairingCode::Table)
                    .drop_column(PairingCode::AttemptsLeft)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(PairingCode::Table)
                    .drop_column(PairingCode::CodeLength)
                    .to_owned(),
            )
            .await
    }
}

// Declared again rather than imported. A migration is a record of what the
// schema was at one moment, and importing an ident a later migration renames
// would rewrite history.
#[derive(DeriveIden)]
enum PairingCode {
    Table,
    ConsumedAt,
    AttemptsLeft,
    CodeLength,
}
