use std::fmt::Error;

use std::net::SocketAddr;
use std::path::PathBuf;
use clap::Parser;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{extract::DefaultBodyLimit, http::StatusCode, routing::post, Json, Router};
use sea_orm::entity::prelude::*;

use sea_orm::{Database, DatabaseConnection, DbErr};
use sea_orm::ActiveValue::Set;
use serde_json::{json, Value};

use uuid::Uuid;

use dirk::dirk_api::{
    DirkReason, DirkResult, FileUpdateRequest, QuickScanBulkRequest, QuickScanBulkResult,
    QuickScanResult,
};
use dirk::hank::{build_sigs_from_file, Signature};
use dirk::entities::{prelude::*, *};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser, default_value_t = SocketAddr::from(([127, 0, 0, 1], 3000)))]
    listen: SocketAddr,
    #[clap(short, long, value_parser, default_value_t = String::from("signatures.json"))]
    signatures: String,
}

const DATABASE_URL: &str = "mysql://dirk:ahghei4phahk5Ooc@localhost:3306/dirk";

async fn quick_scan(
    State(state): State<DirkState>,
    axum::Json(bulk_payload): axum::Json<QuickScanBulkRequest>,
) -> impl IntoResponse {
    let mut results: Vec<QuickScanResult> = Vec::new();
    let code = StatusCode::OK;
    for payload in bulk_payload.requests {
        let file_path = payload.file_name;
        let result =
            match dirk::hank::analyze_file_data(&payload.file_contents, &file_path, &state.sigs) {
                Ok(scanresult) => QuickScanResult {
                    file_name: file_path,
                    result: scanresult.status,
                    reason: DirkReason::LegacyRule,
                    signature: scanresult.signature,
                },
                Err(e) => {
                    eprintln!("Error encountered: {e}");
                    QuickScanResult {
                        file_name: file_path,
                        result: DirkResult::Inconclusive,
                        reason: DirkReason::InternalError,
                        signature: None,
                    }
                }
            };
        results.push(result);
    }
    let id = Uuid::new_v4();
    let bulk_result = QuickScanBulkResult { id, results };
    (code, axum::Json(bulk_result)).into_response()
}

#[derive(Clone)]
struct DirkState {
    sigs: Vec<Signature>,
}
//async fn full_scan() {}
//async fn health_check() {}

async fn get_db() -> Result<DatabaseConnection, DbErr> {
    Database::connect(DATABASE_URL).await
}

async fn list_known_files(State(_state): State<DirkState>) -> Json<Value> {
    let db = get_db().await.unwrap();
    let files: Vec<files::Model> = Files::find().all(&db).await.unwrap();
    Json(json!(files))
}

async fn update_file(mut rec: files::Model, req: FileUpdateRequest) -> Result<(), Error> {
    let db = get_db().await.unwrap();
    rec.last_updated = DateTime::default();
    rec.file_status = req.file_status;
    let rec: files::ActiveModel = rec.into();
    rec.update(&db).await.unwrap();
    Ok(())
}

async fn create_file(req: FileUpdateRequest) -> Result<(), Error> {
    let db = get_db().await.unwrap();
    let file = files::ActiveModel {
        sha256sum: Set(req.checksum),
        file_status: Set(req.file_status),
        ..Default::default()
    };
    let _file = file.insert(&db).await.unwrap();
    Ok(())
}

async fn update_file_api(
    State(_state): State<DirkState>,
    Json(file): Json<FileUpdateRequest>,
) -> impl IntoResponse {
    let db = get_db().await.unwrap();
    let file_record: Option<files::Model> = Files::find()
        .filter(files::Column::Sha256sum.contains(&file.checksum))
        .one(&db)
        .await
        .unwrap();
    match file_record {
        Some(rec) => update_file(rec, file).await.unwrap(),
        None => create_file(file).await.unwrap(),
    }
}

#[tokio::main()]
async fn main() {
    let args = Args::parse();
    let sigs = build_sigs_from_file(PathBuf::from(args.signatures)).unwrap();
    let app_state = DirkState { sigs };
    let scanner_app = Router::new()
        //        .route("/health-check", get(health_check))
        //        .route("/scanner/full", post(full_scan));
        .route("/files/update", post(update_file_api))
        .route("/files/list", get(list_known_files))
        .route("/scanner/quick", post(quick_scan))
        .layer(DefaultBodyLimit::disable())
        .with_state(app_state);
    let addr: SocketAddr = args.listen;
    axum::Server::bind(&addr)
        .serve(scanner_app.into_make_service())
        .await
        .unwrap();
}
