use std::net::SocketAddr;
use std::path::PathBuf;

use axum::{http::StatusCode, Router, routing::post};

use axum::extract::State;
use axum::response::{IntoResponse};

use clap::Parser;

use uuid::Uuid;

use dirk::dirk_api::{QuickScanRequest, QuickScanResult, DirkReason, DirkResult};
use dirk::hank::{build_sigs_from_file, Signature};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser, default_value_t = String::from("signatures.json"))]
    signatures: String,
}

async fn quick_scan(State(state): State<DirkState>, axum::Json(payload): axum::Json<QuickScanRequest>) -> impl IntoResponse {
    let id = Uuid::new_v4();
    let result: QuickScanResult;
    let mut code = StatusCode::OK;
    let file_path = payload.file_name;
    match dirk::hank::analyze_file_data(&payload.file_contents, &file_path, &state.sigs) {
        Ok(scanresult) => {
            result = QuickScanResult {
                id,
                result: scanresult.status,
                reason: DirkReason::LegacyRule,
            };
        },
        Err(e) => {
            eprintln!("Error encountered: {e}");
            result = QuickScanResult {
                id,
                result: DirkResult::Inconclusive,
                reason: DirkReason::InternalError,
            };
            code = StatusCode::INTERNAL_SERVER_ERROR;
        },
    };
    (code, axum::Json(result)).into_response()
}

#[derive(Clone)]
struct DirkState {
    sigs: Vec<Signature>,
}
//async fn full_scan() {}
//async fn health_check() {}

#[tokio::main()]
async fn main() {
    let args = Args::parse();
    let sigs = build_sigs_from_file(PathBuf::from(args.signatures)).unwrap();
    let app_state = DirkState {
        sigs,
    };
    let scanner_app = Router::new()
        //        .route("/health-check", get(health_check))
        //        .route("/scanner/full", post(full_scan));
        .route("/scanner/quick", post(quick_scan))
        .with_state(app_state);
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    axum::Server::bind(&addr)
        .serve(scanner_app.into_make_service())
        .await
        .unwrap();
}
