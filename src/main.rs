use axum::{
    Router,
    routing::{get, post},
};

async fn quick_scan() {}
async fn full_scan() {}
async fn health_check() {}

#[tokio::main()]
async fn main() {
    let scanner_app = Router::new()
        .route("/health-check", get(health_check))
        .route("/scanner/quick", post(quick_scan))
        .route("/scanner/full", post(full_scan));
    println!("Hello, world!");
}
