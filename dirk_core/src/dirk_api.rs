use std::collections::HashMap;
use std::default::Default;
use std::fmt::Error;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::error_handling::HandleErrorLayer;
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{extract::DefaultBodyLimit, http::StatusCode, routing::post, BoxError, Json, Router};
use sea_orm::entity::prelude::*;
use sea_orm::ActiveValue::Set;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbErr, Statement};
use serde_json::{json, Value};
use tower::ServiceBuilder;
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tower_http::LatencyUnit;
use tracing::Level;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

use crate::container;
use crate::entities::prelude::*;
use crate::entities::sea_orm_active_enums::*;
use crate::entities::*;
use crate::errors::DirkError;
use crate::hank::analyze_file_data;
use crate::models::dirk::*;

pub fn build_router(app_state: Arc<DirkState>) -> Result<Router, DirkError> {
    let _ = tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .try_init();
    Ok(Router::new()
        .route("/health-check", get(health_check))
        .route("/scanner/quick", post(quick_scan))
        .route("/scanner/full", post(full_scan))
        .route("/scanner/dynamic", post(dynamic_scan_api))
        .route("/files/update", post(update_file_api))
        .route("/files/list", get(list_known_files))
        .route("/files/get/:sha1sum", get(get_file_status_api))
        .layer(DefaultBodyLimit::disable())
        // Add middleware to all routes
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(|error: BoxError| async move {
                    if error.is::<tower::timeout::error::Elapsed>() {
                        Ok(StatusCode::REQUEST_TIMEOUT)
                    } else {
                        Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("Unhandled internal error: {}", error),
                        ))
                    }
                }))
                .timeout(Duration::from_secs(120))
                .into_inner(),
        )
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().include_headers(true))
                .on_request(DefaultOnRequest::new().level(Level::INFO))
                .on_response(
                    DefaultOnResponse::new()
                        .level(Level::INFO)
                        .latency_unit(LatencyUnit::Micros),
                ),
        )
        .with_state(app_state))
}
const DATABASE_URL: &str = "mysql://dirk:ahghei4phahk5Ooc@localhost:3306/dirk";

///Full scan inspects the list of known sha1 digests as well as scanning file content
async fn full_scan(
    State(state): State<Arc<DirkState>>,
    Json(bulk_payload): Json<ScanBulkRequest>,
) -> impl IntoResponse {
    let mut results: Vec<ScanResult> = Vec::new();
    let code = StatusCode::OK;
    for payload in bulk_payload.requests {
        let file_path = payload.file_name;
        if !payload.skip_cache {
            if let Some(file) = fetch_status(&state.db, &payload.sha1sum).await {
                let result = ScanResult {
                    file_names: Vec::from([file_path]),
                    sha1sum: file.sha1sum,
                    result: match file.file_status {
                        FileStatus::Good => DirkResultClass::OK,
                        FileStatus::Bad => DirkResultClass::Bad,
                        FileStatus::Whitelisted => DirkResultClass::OK,
                        FileStatus::Blacklisted => DirkResultClass::Bad,
                    },
                    reason: DirkReason::Cached,
                    ..Default::default()
                };
                results.push(result);
                continue;
            }
        }
        // We only reach this point if `skip_cache` wasn't set on the request AND
        // if the file wasn't able to be fetched from our cache.
        let result = match analyze_file_data(
            &payload.file_contents.unwrap_or_default(),
            &file_path,
            &state.sigs,
        ) {
            Ok(scanresult) => ScanResult {
                file_names: Vec::from([file_path]),
                sha1sum: payload.sha1sum.clone(),
                result: scanresult.status,
                reason: DirkReason::LegacyRule,
                signature: scanresult.signature,
                ..Default::default()
            },
            Err(e) => {
                eprintln!("Error encountered: {e}");
                ScanResult {
                    file_names: Vec::from([file_path]),
                    sha1sum: payload.sha1sum.clone(),
                    result: DirkResultClass::Inconclusive,
                    reason: DirkReason::InternalError,
                    ..Default::default()
                }
            }
        };
        if let DirkResultClass::Bad = result.result {
            let csum = result.sha1sum.clone();
            let file = FileUpdateRequest {
                checksum: csum,
                file_status: FileStatus::Bad,
            };
            let _res = create_or_update_file(file, &state.db).await;
        }
        results.push(result);
    }
    let id = Uuid::new_v4();
    let bulk_result = ScanBulkResult { id, results };
    (code, Json(bulk_result)).into_response()
}

///Quick scan that only looks up SHA1 digests against the database
async fn quick_scan(
    State(state): State<Arc<DirkState>>,
    Json(bulk_payload): Json<ScanBulkRequest>,
) -> impl IntoResponse {
    let code = StatusCode::OK;
    let db = &state.db;

    let mut sums: Vec<String> = Vec::new();
    let mut sum_map: HashMap<String, Vec<PathBuf>> = HashMap::new();

    for req in bulk_payload.requests {
        let file_name = req.file_name.clone();
        sum_map
            .entry(req.sha1sum.to_string())
            .and_modify(|this_map| this_map.push(req.file_name))
            .or_insert_with(|| Vec::from([file_name]));
        sums.push(req.sha1sum);
    }

    let files: Vec<files::Model> = Files::find()
        .filter(files::Column::Sha1sum.is_in(sums))
        .all(db)
        .await
        .unwrap();

    let results = files
        .into_iter()
        .map(|file| {
            let sha1sum = file.sha1sum.clone();
            let status = file.file_status;
            let class = match status {
                FileStatus::Bad | FileStatus::Blacklisted => DirkResultClass::Bad,
                FileStatus::Good | FileStatus::Whitelisted => DirkResultClass::OK,
            };
            ScanResult {
                file_names: sum_map[&sha1sum].clone(),
                cache_detail: Some(status),
                reason: DirkReason::Cached,
                result: class,
                sha1sum: file.sha1sum,
                ..Default::default()
            }
        })
        .collect();
    let bulk_result = ScanBulkResult {
        id: Uuid::new_v4(),
        results,
    };

    (code, Json(bulk_result)).into_response()
}

async fn health_check(State(state): State<Arc<DirkState>>) -> impl IntoResponse {
    let db = &state.db;
    let stmt = Statement::from_string(
        db.get_database_backend(),
        "select count(*) as file_num from files".to_owned(),
    );
    if let Ok(result) = db.query_one(stmt).await {
        (
            StatusCode::OK,
            format!(
                "Hi! All's good here. {:#?}",
                result.unwrap().try_get::<i64>("", "file_num").unwrap()
            ),
        )
            .into_response()
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database connection failed",
        )
            .into_response()
    }
}

pub async fn get_db() -> Result<DatabaseConnection, DbErr> {
    Database::connect(DATABASE_URL).await
}

///Dump a listing of all known files
async fn list_known_files(State(state): State<Arc<DirkState>>) -> Json<Value> {
    let db = &state.db;
    let files: Vec<files::Model> = Files::find().all(db).await.unwrap();
    Json(json!(files))
}

///Fetch a single File record from the database
async fn fetch_status(db: &DatabaseConnection, csum: &str) -> Option<files::Model> {
    Files::find()
        .filter(files::Column::Sha1sum.eq(csum))
        .one(db)
        .await
        .unwrap()
}

///API to run a dynamic analysis on a single file
async fn dynamic_scan_api(
    State(state): State<Arc<DirkState>>,
    Json(payload): Json<ScanBulkRequest>,
) -> impl IntoResponse {
    let _db = &state.db;
    let scan_id = Uuid::new_v4();
    let mut results: Vec<ScanResult> = Vec::new();
    for request in payload.requests {
        if let Ok(tmp_dir) = tempfile::Builder::new()
            .prefix(&scan_id.to_string())
            .tempdir()
        {
            if let Ok(test_result) = container::examine_one(tmp_dir, &request).await {
                let result = ScanResult {
                    file_names: vec![request.file_name],
                    sha1sum: request.sha1sum,
                    result: match test_result.len() {
                        0 => DirkResultClass::OK,
                        _ => DirkResultClass::Bad,
                    },
                    reason: DirkReason::DynamicRule,
                    dynamic_results: Some(test_result.into_iter().collect()),
                    ..Default::default()
                };
                results.push(result);
            }
        }
    }
    let bulk_result = ScanBulkResult {
        id: Uuid::new_v4(),
        results,
    };
    (StatusCode::OK, Json(bulk_result)).into_response()
}
///API to retrieve a single file record
async fn get_file_status_api(
    State(state): State<Arc<DirkState>>,
    Path(sha1sum): Path<String>,
) -> Json<Value> {
    let db = &state.db;
    println!("Fetching file status for {}", &sha1sum);
    Json(json!(fetch_status(db, &sha1sum).await))
}

///Update a file record in the database
async fn update_file(
    rec: files::Model,
    req: FileUpdateRequest,
    db: &DatabaseConnection,
) -> Result<(), Error> {
    let mut rec: files::ActiveModel = rec.into();
    rec.last_updated = Set(DateTime::default());
    rec.file_status = Set(req.file_status);
    rec.update(db).await.unwrap();
    Ok(())
}

///Create a new fie record in the database
async fn create_file(req: FileUpdateRequest, db: &DatabaseConnection) -> Result<(), DirkError> {
    let file = files::ActiveModel {
        sha1sum: Set(req.checksum),
        file_status: Set(req.file_status),
        ..Default::default()
    };
    println!("Creating new file");
    let _file = file.insert(db).await?;
    Ok(())
}

///Wrapper to create or update a file record
async fn create_or_update_file(
    file: FileUpdateRequest,
    db: &DatabaseConnection,
) -> impl IntoResponse {
    let csum = file.checksum.clone();
    let file_record: Option<files::Model> = Files::find()
        .filter(files::Column::Sha1sum.eq(csum))
        .one(db)
        .await
        .unwrap();
    match file_record {
        Some(rec) => update_file(rec, file, db).await.unwrap(),
        None => create_file(file, db).await.unwrap(),
    }
}

///API endpoint to update a file record
async fn update_file_api(
    State(state): State<Arc<DirkState>>,
    Json(file): Json<FileUpdateRequest>,
) -> impl IntoResponse {
    let db = &state.db;
    create_or_update_file(file, db).await
}
