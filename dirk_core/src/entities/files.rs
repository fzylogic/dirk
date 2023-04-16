//! `SeaORM` Entity. Generated by sea-orm-codegen 0.11.2

use super::sea_orm_active_enums::FileStatus;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "files")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub sha1sum: String,
    pub first_seen: DateTime,
    pub last_seen: DateTime,
    pub last_updated: DateTime,
    pub file_status: FileStatus,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::file_rule_match::Entity")]
    FileRuleMatch,
}

impl Related<super::file_rule_match::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FileRuleMatch.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
