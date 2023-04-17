#[allow(unused_imports)]
use sea_orm::*;

#[allow(unused_imports)]
use dirk_core::entities::sea_orm_active_enums::*;
#[allow(unused_imports)]
use dirk_core::entities::*;

#[cfg(feature = "mock")]
pub fn prepare_mock_db() -> DatabaseConnection {
    MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results(vec![
            //`dirk`
            vec![files::Model {
                id: 0,
                sha1sum: "2d69120f4a37384f5b712c447e7bd630eda348a5ad96ce3356900d6410935b56"
                    .to_string(),
                first_seen: Default::default(),
                last_seen: Default::default(),
                last_updated: Default::default(),
                file_status: FileStatus::Good,
                signatures: vec![],
            }],
            //`DreamHost`
            vec![files::Model {
                id: 1,
                sha1sum: "2b998d019098754f1c0ae7eeb21fc4e673c6271b1d17593913ead73f5be772f1"
                    .to_string(),
                first_seen: Default::default(),
                last_seen: Default::default(),
                last_updated: Default::default(),
                file_status: FileStatus::Good,
                signatures: vec![],
            }],
            //`ED2ho4ura0vaiJ4j`
            vec![files::Model {
                id: 4,
                sha1sum: "c7bc91edeab8f0a35159d314df390d87c6be1d07aa31a9489a3a878a217eea32"
                    .to_string(),
                first_seen: Default::default(),
                last_seen: Default::default(),
                last_updated: Default::default(),
                file_status: FileStatus::Good,
                signatures: vec![],
            }],
            //`vieC4aezai7ahphu`
            vec![files::Model {
                id: 6,
                sha1sum: "ff2c61af201e1daa2127bfe058bb9d13ea89d0ebb6c65e80837d304abd4d3091"
                    .to_string(),
                first_seen: Default::default(),
                last_seen: Default::default(),
                last_updated: Default::default(),
                file_status: FileStatus::Good,
                signatures: vec![],
            }],
        ])
        .append_exec_results(vec![
            MockExecResult {
                last_insert_id: 6,
                rows_affected: 1,
            },
            MockExecResult {
                last_insert_id: 6,
                rows_affected: 5,
            },
        ])
        .into_connection()
}
