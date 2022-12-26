use std::collections::HashMap;
use std::fmt::Error;

use axum::error_handling::HandleErrorLayer;
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{BoxError, extract::DefaultBodyLimit, http::StatusCode, Json, Router, routing::post};
use clap::Parser;
use sea_orm::entity::prelude::*;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use sea_orm::ActiveValue::Set;
use sea_orm::{Database, DatabaseConnection, DbErr};
use serde_json::{json, Value};
use tower::ServiceBuilder;
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tower_http::LatencyUnit;
use tracing::Level;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use uuid::Uuid;

use dirk_core::dirk_api::{
    DirkReason, DirkResultClass, FileUpdateRequest, ScanBulkRequest, ScanBulkResult, ScanResult,
};

use dirk_core::entities::*;
use dirk_core::entities::prelude::*;
use dirk_core::entities::sea_orm_active_enums::*;
use dirk_core::hank::*;
use dirk_core::dirk_api::*;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser, default_value_t = SocketAddr::from(([127, 0, 0, 1], 3000)))]
    listen: SocketAddr,
    #[clap(short, long, value_parser, default_value_t = String::from("signatures.json"))]
    signatures: String,
}

const DATABASE_URL: &str = "mysql://dirk:ahghei4phahk5Ooc@localhost:3306/dirk";

///Full scan inspects the list of known sha256 digests as well as scanning file content
async fn full_scan(
    State(state): State<DirkState>,
    Json(bulk_payload): Json<ScanBulkRequest>,
) -> impl IntoResponse {
    let mut results: Vec<ScanResult> = Vec::new();
    let code = StatusCode::OK;
    for payload in bulk_payload.requests {
        let file_path = payload.file_name;
        if let Some(file) = fetch_status(&state.db, payload.sha256sum.clone()).await {
            let result = ScanResult {
                file_names: Vec::from([file_path]),
                sha256sum: file.sha256sum,
                result: match file.file_status {
                    FileStatus::Good => DirkResultClass::OK,
                    FileStatus::Bad => DirkResultClass::Bad,
                    FileStatus::Whitelisted => DirkResultClass::OK,
                    FileStatus::Blacklisted => DirkResultClass::Bad,
                },
                reason: DirkReason::Cached,
                cache_detail: None,
                signature: None,
            };
            results.push(result);
        } else {
            let result = match analyze_file_data(
                &payload.file_contents.unwrap_or_default(),
                &file_path,
                &state.sigs,
            ) {
                Ok(scanresult) => ScanResult {
                    file_names: Vec::from([file_path]),
                    sha256sum: payload.sha256sum.clone(),
                    result: scanresult.status,
                    reason: DirkReason::LegacyRule,
                    cache_detail: None,
                    signature: scanresult.signature,
                },
                Err(e) => {
                    eprintln!("Error encountered: {e}");
                    ScanResult {
                        file_names: Vec::from([file_path]),
                        sha256sum: payload.sha256sum.clone(),
                        result: DirkResultClass::Inconclusive,
                        reason: DirkReason::InternalError,
                        cache_detail: None,
                        signature: None,
                    }
                }
            };
            if let DirkResultClass::Bad = result.result {
                let csum = result.sha256sum.clone();
                let file = FileUpdateRequest {
                    checksum: csum,
                    file_status: FileStatus::Bad,
                };
                let _res = create_or_update_file(file, &state.db).await;
            }
            results.push(result);
        }
    }
    let id = Uuid::new_v4();
    let bulk_result = ScanBulkResult { id, results };
    (code, Json(bulk_result)).into_response()
}

///Quick scan that only looks up sha256 digests against the database
async fn quick_scan(
    State(state): State<DirkState>,
    Json(bulk_payload): Json<ScanBulkRequest>,
) -> impl IntoResponse {
    let code = StatusCode::OK;
    let db = state.db;

    let mut sums: Vec<String> = Vec::new();
    let mut sum_map: HashMap<String, Vec<PathBuf>> = HashMap::new();

    for req in bulk_payload.requests {
        let file_name = req.file_name.clone();
        sum_map
            .entry(req.sha256sum.to_string())
            .and_modify(|this_map| this_map.push(req.file_name))
            .or_insert_with(|| Vec::from([file_name]));
        sums.push(req.sha256sum);
    }

    let files: Vec<files::Model> = Files::find()
        .filter(files::Column::Sha256sum.is_in(sums))
        .all(&db)
        .await
        .unwrap();

    let results = files
        .into_iter()
        .map(|file| {
            let sha256sum = file.sha256sum.clone();
            let status = file.file_status;
            let class = match status {
                FileStatus::Bad | FileStatus::Blacklisted => DirkResultClass::Bad,
                FileStatus::Good | FileStatus::Whitelisted => DirkResultClass::OK,
            };
            ScanResult {
                file_names: sum_map[&sha256sum].clone(),
                cache_detail: Some(status),
                reason: DirkReason::Cached,
                signature: None,
                result: class,
                sha256sum: file.sha256sum,
            }
        })
        .collect();
    let id = Uuid::new_v4();
    let bulk_result = ScanBulkResult { id, results };
    (code, Json(bulk_result)).into_response()
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "Hi!").into_response()
}

async fn get_db() -> Result<DatabaseConnection, DbErr> {
    Database::connect(DATABASE_URL).await
}

///Dump a listing of all known files
async fn list_known_files(State(state): State<DirkState>) -> Json<Value> {
    let db = state.db;
    let files: Vec<files::Model> = Files::find().all(&db).await.unwrap();
    Json(json!(files))
}

///Fetch a single File record from the database
async fn fetch_status(db: &DatabaseConnection, csum: String) -> Option<files::Model> {
    Files::find()
        .filter(files::Column::Sha256sum.eq(csum))
        .one(db)
        .await
        .unwrap()
}

///API to retrieve a single file record
async fn get_file_status_api(
    State(state): State<DirkState>,
    Path(sha256sum): Path<String>,
) -> Json<Value> {
    let db = state.db;
    println!("Fetching file status for {}", &sha256sum);
    Json(json!(fetch_status(&db, sha256sum).await))
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
async fn create_file(req: FileUpdateRequest, db: &DatabaseConnection) -> Result<(), Error> {
    let file = files::ActiveModel {
        sha256sum: Set(req.checksum),
        file_status: Set(req.file_status),
        ..Default::default()
    };
    println!("Creating new file");
    let _file = file.insert(db).await.unwrap();
    Ok(())
}

///Wrapper to create or update a file record
async fn create_or_update_file(
    file: FileUpdateRequest,
    db: &DatabaseConnection,
) -> impl IntoResponse {
    let csum = file.checksum.clone();
    let file_record: Option<files::Model> = Files::find()
        .filter(files::Column::Sha256sum.eq(csum))
        .one(db)
        .await
        .unwrap();
    match file_record {
        Some(rec) => update_file(rec, file, &db).await.unwrap(),
        None => create_file(file, &db).await.unwrap(),
    }
}

///API endpoint to update a file record
async fn update_file_api(
    State(state): State<DirkState>,
    Json(file): Json<FileUpdateRequest>,
) -> impl IntoResponse {
    let db = state.db;
    create_or_update_file(file, &db).await
}

fn build_router(app_state: DirkState) -> Router {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    Router::new()
        .route("/health-check", get(health_check))
        .route("/scanner/quick", post(quick_scan))
        .route("/scanner/full", post(full_scan))
        .route("/files/update", post(update_file_api))
        .route("/files/list", get(list_known_files))
        .route("/files/get/:sha256sum", get(get_file_status_api))
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
        .with_state(app_state)
}

#[tokio::main()]
async fn main() {
    let args = Args::parse();
    let db = get_db().await.unwrap();
    let sigs = build_sigs_from_file(PathBuf::from(args.signatures)).unwrap();
    let app_state = DirkState { sigs, db };

    let addr: SocketAddr = args.listen;
    let scanner_app = build_router(app_state);
    axum::Server::bind(&addr)
        .serve(scanner_app.into_make_service())
        .await
        .unwrap();
}

