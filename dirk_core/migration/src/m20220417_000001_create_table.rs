use crate::sea_orm::EntityName;
use dirk_core::entities::file_rule_match;
use dirk_core::entities::files;
use dirk_core::entities::sea_orm_active_enums::FileStatus as FileStatusEnum;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts

        manager
            .create_table(
                Table::create()
                    .table(files::Entity.table_ref())
                    .if_not_exists()
                    .col(
                        ColumnDef::new(files::Column::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(files::Column::Sha1sum).string().not_null())
                    .col(
                        ColumnDef::new(files::Column::FirstSeen)
                            .date_time()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(files::Column::LastSeen)
                            .date_time()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(files::Column::LastUpdated)
                            .date_time()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(files::Column::FileStatus)
                            .enumeration(files::Column::FileStatus, FileStatusEnum::iden_values()),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(file_rule_match::Entity.table_ref())
                    .if_not_exists()
                    .col(
                        ColumnDef::new(file_rule_match::Column::FileId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(file_rule_match::Column::RuleName)
                            .string()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .name("unique_combo")
                            .table(file_rule_match::Entity.table_ref())
                            .col(file_rule_match::Column::FileId)
                            .col(file_rule_match::Column::RuleName),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("file_name_id")
                            .to(files::Entity.table_ref(), files::Column::Id)
                            .from(
                                file_rule_match::Entity.table_ref(),
                                file_rule_match::Column::FileId,
                            )
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        manager
            .drop_table(
                Table::drop()
                    .table(file_rule_match::Entity.table_ref())
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(files::Entity.table_ref()).to_owned())
            .await
    }
}

// Learn more at https://docs.rs/sea-query#iden
// #[derive(Iden)]
// enum Files {
//     Table,
//     Id,
//     Sha1sum,
//     FirstSeen,
//     LastSeen,
//     LastUpdated,
//     FileStatus,
// }
