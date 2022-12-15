use std::net::SocketAddr;
use std::path::PathBuf;

use axum::{extract::DefaultBodyLimit, http::StatusCode, routing::post, Router};

use axum::extract::State;
use axum::response::IntoResponse;

use clap::Parser;

use uuid::Uuid;

use dirk::dirk_api::{
    DirkReason, DirkResult, QuickScanBulkRequest, QuickScanBulkResult, QuickScanResult,
};
use dirk::hank::{build_sigs_from_file, Signature};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser, default_value_t = SocketAddr::from(([127, 0, 0, 1], 3000)))]
    listen: SocketAddr,
    #[clap(short, long, value_parser, default_value_t = String::from("signatures.json"))]
    signatures: String,
}

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

#[tokio::main()]
async fn main() {
    let args = Args::parse();
    let sigs = build_sigs_from_file(PathBuf::from(args.signatures)).unwrap();
    let app_state = DirkState { sigs };
    let scanner_app = Router::new()
        //        .route("/health-check", get(health_check))
        //        .route("/scanner/full", post(full_scan));
        .route("/scanner/quick", post(quick_scan))
        .layer(DefaultBodyLimit::disable())
        .with_state(app_state);
    let addr: SocketAddr = args.listen;
    axum::Server::bind(&addr)
        .serve(scanner_app.into_make_service())
        .await
        .unwrap();
}
