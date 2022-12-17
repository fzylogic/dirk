use std::fmt::Error;

use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{extract::DefaultBodyLimit, http::StatusCode, routing::post, Json, Router};
use clap::Parser;
use sea_orm::entity::prelude::*;
use std::net::SocketAddr;
use std::path::PathBuf;

use sea_orm::ActiveValue::Set;
use sea_orm::{Database, DatabaseConnection, DbErr};
use serde_json::{json, Value};

use uuid::Uuid;

use dirk::dirk_api::{
    DirkReason, DirkResult, FileUpdateRequest, FullScanBulkRequest, FullScanBulkResult,
    FullScanResult, QuickScanBulkRequest, QuickScanBulkResult, QuickScanResult,
};
use dirk::entities::files::Model;
use dirk::entities::{prelude::*, *};
use dirk::hank::{build_sigs_from_file, Signature};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser, default_value_t = SocketAddr::from(([127, 0, 0, 1], 3000)))]
    listen: SocketAddr,
    #[clap(short, long, value_parser, default_value_t = String::from("signatures.json"))]
    signatures: String,
}

const DATABASE_URL: &str = "mysql://dirk:ahghei4phahk5Ooc@localhost:3306/dirk";

async fn full_scan(
    State(state): State<DirkState>,
    Json(bulk_payload): Json<FullScanBulkRequest>,
) -> impl IntoResponse {
    let mut results: Vec<FullScanResult> = Vec::new();
    let code = StatusCode::OK;
    for payload in bulk_payload.requests {
        let file_path = payload.file_name;
        let result =
            match dirk::hank::analyze_file_data(&payload.file_contents, &file_path, &state.sigs) {
                Ok(scanresult) => FullScanResult {
                    file_name: file_path,
                    result: scanresult.status,
                    reason: DirkReason::LegacyRule,
                    signature: scanresult.signature,
                },
                Err(e) => {
                    eprintln!("Error encountered: {e}");
                    FullScanResult {
                        file_name: file_path,
                        result: DirkResult::Inconclusive,
                        reason: DirkReason::InternalError,
                        signature: None,
                    }
                }
            };
        results.push(result);
    }
    //TODO Store the results in the database
    let id = Uuid::new_v4();
    let bulk_result = FullScanBulkResult { id, results };
    (code, axum::Json(bulk_result)).into_response()
}

async fn quick_scan(
    State(state): State<DirkState>,
    Json(bulk_payload): Json<QuickScanBulkRequest>,
) -> impl IntoResponse {
    //let mut results: Vec<FullScanResult> = Vec::new();
    let code = StatusCode::OK;
    let db = state.db;
    println!("Initiating quick scan");
    let sums: Vec<String> = bulk_payload
        .requests
        .into_iter()
        .map(|req| req.sha256sum.as_str().to_owned())
        .collect();

    let files: Vec<Model> = Files::find()
        .filter(files::Column::Sha256sum.is_in(sums))
        .all(&db)
        .await
        .unwrap();
    let results = files
        .into_iter()
        .map(|file| QuickScanResult {
            sha256sum: file.sha256sum,
            result: file.file_status,
        })
        .collect();
    //println!("{:?}", files);
    let bulk_result = QuickScanBulkResult { results };
    (code, Json(bulk_result)).into_response()
}

#[derive(Clone)]
struct DirkState {
    sigs: Vec<Signature>,
    db: DatabaseConnection,
}

//async fn health_check() {}

async fn get_db() -> Result<DatabaseConnection, DbErr> {
    Database::connect(DATABASE_URL).await
}

async fn list_known_files(State(state): State<DirkState>) -> Json<Value> {
    let db = state.db;
    let files: Vec<files::Model> = Files::find().all(&db).await.unwrap();
    Json(json!(files))
}

async fn update_file(
    mut rec: files::Model,
    req: FileUpdateRequest,
    db: DatabaseConnection,
) -> Result<(), Error> {
    rec.last_updated = DateTime::default();
    rec.file_status = req.file_status;
    let rec: files::ActiveModel = rec.into();
    rec.update(&db).await.unwrap();
    Ok(())
}

async fn create_file(req: FileUpdateRequest, db: DatabaseConnection) -> Result<(), Error> {
    let file = files::ActiveModel {
        sha256sum: Set(req.checksum),
        file_status: Set(req.file_status),
        ..Default::default()
    };
    let _file = file.insert(&db).await.unwrap();
    Ok(())
}

async fn update_file_api(
    State(state): State<DirkState>,
    Json(file): Json<FileUpdateRequest>,
) -> impl IntoResponse {
    let db = state.db;
    let file_record: Option<files::Model> = Files::find()
        .filter(files::Column::Sha256sum.contains(&file.checksum))
        .one(&db)
        .await
        .unwrap();
    match file_record {
        Some(rec) => update_file(rec, file, db).await.unwrap(),
        None => create_file(file, db).await.unwrap(),
    }
}

#[tokio::main()]
async fn main() {
    let args = Args::parse();
    let db = get_db().await.unwrap();
    let sigs = build_sigs_from_file(PathBuf::from(args.signatures)).unwrap();
    let app_state = DirkState { sigs, db };
    let scanner_app = Router::new()
        //        .route("/health-check", get(health_check))
        .route("/scanner/quick", post(quick_scan))
        .route("/scanner/full", post(full_scan))
        .route("/files/update", post(update_file_api))
        .route("/files/list", get(list_known_files))
        //       .route("/files/get/:sha256sum", get(get_file_status))
        .layer(DefaultBodyLimit::disable())
        .with_state(app_state);
    let addr: SocketAddr = args.listen;
    axum::Server::bind(&addr)
        .serve(scanner_app.into_make_service())
        .await
        .unwrap();
}
