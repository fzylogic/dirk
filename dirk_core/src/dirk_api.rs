use std::collections::HashMap;
use std::default::Default;
use std::fmt::Error;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::error_handling::HandleErrorLayer;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{http::StatusCode, routing::post, BoxError, Json, Router};
use chrono::offset::Utc;
use http::Method;
use rayon::prelude::*;
use sea_orm::entity::prelude::*;
use sea_orm::ActiveValue::Set;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbErr, Statement};
use serde_json::{json, Value};
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
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
    let cors = CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods([Method::GET, Method::POST])
        // allow requests from any origin
        .allow_origin(Any);
    Ok(Router::new()
        .route("/health-check", get(health_check))
        .route("/scanner/quick", post(quick_scan))
        .route("/scanner/full", post(full_scan))
        .route("/scanner/dynamic", post(dynamic_scan_api))
        .route("/files/update", post(update_file_api))
        .route("/files/list", get(list_known_files))
        .route("/files/get/:sha1sum", get(get_file_status_api))
        .layer(cors)
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

impl ScanRequest {
    fn process(&self, state: &Arc<DirkState>) -> ScanResult {
        let file_path = self.file_name.clone();
        let result = match analyze_file_data(
            self.file_contents.as_ref().unwrap_or(&"".to_string()),
            &file_path,
            &state.sigs,
        ) {
            Ok(scanresult) => ScanResult {
                file_names: Vec::from([file_path]),
                sha1sum: self.sha1sum.clone(),
                result: scanresult.status,
                reason: DirkReason::LegacyRule,
                signature: scanresult.signature,
                ..Default::default()
            },
            Err(e) => {
                eprintln!("Error encountered: {}", e);
                ScanResult {
                    file_names: Vec::from([file_path]),
                    sha1sum: self.sha1sum.clone(),
                    result: DirkResultClass::Inconclusive,
                    reason: DirkReason::InternalError,
                    ..Default::default()
                }
            }
        };
        result
    }
}

///Full scan inspects the list of known sha1 digests as well as scanning file content
async fn full_scan(
    State(state): State<Arc<DirkState>>,
    Json(bulk_payload): Json<ScanBulkRequest>,
) -> impl IntoResponse {
    let code = StatusCode::OK;
    let (sums, sum_map) = map_reqs(&bulk_payload.requests);

    let mut cached: Vec<ScanResult> = match bulk_payload.skip_cache {
        true => Vec::new(),
        false => {
            let files: Vec<files::Model> = Files::find()
                .filter(files::Column::Sha1sum.is_in(sums.clone()))
                .all(&state.db)
                .await
                .unwrap();
            db_to_results(files, sum_map)
        }
    };
    let s2 = state.clone();
    let cached_sums: Vec<String> = cached.par_iter().map(move |p| p.sha1sum.clone()).collect();
    let mut results: Vec<ScanResult> = bulk_payload
        .requests
        .par_iter()
        .filter(move |p| !cached_sums.contains(&p.sha1sum))
        .map(move |p| p.process(&s2))
        .collect();
    for result in results.iter().filter(|r| r.result == DirkResultClass::Bad) {
        let csum = result.sha1sum.clone();
        let file = FileUpdateRequest {
            checksum: csum,
            file_status: FileStatus::Bad,
        };
        // Update the database with our result
        match create_file(file, &state.db).await {
            Ok(_) => {}
            Err(e) => {
                eprintln!("{:?}", e)
            }
        }
    }
    // ID's not used yet, but will eventually be used for async requests such that
    // a client can come back for their results after submitting a large request
    let id = Uuid::new_v4();
    results.append(&mut cached);
    let bulk_result = ScanBulkResult { id, results };
    (code, Json(bulk_result)).into_response()
}

fn map_reqs(reqs: &Vec<ScanRequest>) -> (Vec<String>, HashMap<String, Vec<PathBuf>>) {
    let mut sums = Vec::new();
    let mut sum_map: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for req in reqs {
        let file_name = req.file_name.clone();
        sum_map
            .entry(req.sha1sum.to_string())
            .and_modify(|this_map| this_map.push(req.file_name.clone()))
            .or_insert_with(|| Vec::from([file_name]));
        sums.push(req.sha1sum.clone());
    }
    (sums, sum_map)
}

fn db_to_results(
    files: Vec<files::Model>,
    sum_map: HashMap<String, Vec<PathBuf>>,
) -> Vec<ScanResult> {
    files
        .into_par_iter()
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
        .collect()
}

///Quick scan that only looks up SHA1 digests against the database
async fn quick_scan(
    State(state): State<Arc<DirkState>>,
    Json(bulk_payload): Json<ScanBulkRequest>,
) -> impl IntoResponse {
    let code = StatusCode::OK;
    let db = &state.db;

    let (sums, sum_map) = map_reqs(&bulk_payload.requests);

    let files: Vec<files::Model> = Files::find()
        .filter(files::Column::Sha1sum.is_in(sums))
        .all(db)
        .await
        .unwrap();

    let results = db_to_results(files, sum_map);

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
    println!("Updating file {}", req.checksum);
    rec.last_updated = Set(Utc::now().naive_utc());
    rec.file_status = Set(req.file_status);
    rec.update(db).await.unwrap();
    Ok(())
}

///Create a new fie record in the database
async fn create_file(req: FileUpdateRequest, db: &DatabaseConnection) -> Result<(), DirkError> {
    println!("Creating new file {}", &req.checksum);
    let file = files::ActiveModel {
        sha1sum: Set(req.checksum),
        last_updated: Set(Utc::now().naive_utc()),
        last_seen: Set(Utc::now().naive_utc()),
        first_seen: Set(Utc::now().naive_utc()),
        file_status: Set(req.file_status),
        ..Default::default()
    };
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
