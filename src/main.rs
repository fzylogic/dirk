use std::net::SocketAddr;
use std::path::PathBuf;

use axum::response::IntoResponse;
use axum::{http::StatusCode, routing::post, Json, Router};
//use base64;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize)]
pub enum Result {
    Bad,
    Inconclusive,
    OK,
}

#[derive(Serialize)]
pub enum Reason {
    LegacyRule,
    None,
}

#[derive(Deserialize)]
struct QuickScanRequest {
    file_name: PathBuf,
    file_contents: String,
}

#[derive(Serialize)]
pub struct QuickScanResult {
    id: Uuid,
    result: Result,
    reason: Reason,
}

async fn quick_scan(Json(_payload): Json<QuickScanRequest>) -> impl IntoResponse {
    let id = Uuid::new_v4();
    let result = QuickScanResult {
        id,
        result: Result::OK,
        reason: Reason::None,
    };
    (StatusCode::OK, Json(result))
}

//async fn full_scan() {}
//async fn health_check() {}

#[tokio::main()]
async fn main() {
    let scanner_app = Router::new()
        //        .route("/health-check", get(health_check))
        .route("/scanner/quick", post(quick_scan));
    //        .route("/scanner/full", post(full_scan));
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    axum::Server::bind(&addr)
        .serve(scanner_app.into_make_service())
        .await
        .unwrap();
    println!("Hello, world!");
}
