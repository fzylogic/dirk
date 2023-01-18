mod prepare_db;

use axum::http::Uri;
use axum::routing::IntoMakeService;
use axum::{Router, Server};
use dirk_core::dirk_api;
use dirk_core::entities::sea_orm_active_enums::*;
use dirk_core::models::dirk;
use dirk_core::models::dirk::{DirkState, FileUpdateRequest};
use dirk_core::models::hank::*;
use hyper::server::conn::AddrIncoming;
use prepare_db::prepare_mock_db;
use std::net::TcpListener;
use std::sync::Arc;

#[test]
fn full_scan_url() {
    let urlbase: Uri = "http://127.0.0.1:3000".parse::<Uri>().unwrap();
    let full_type = dirk::ScanType::Full;
    assert_eq!(
        full_type.url(urlbase),
        "http://127.0.0.1:3000/scanner/full".to_string()
    );
}

#[test]
fn quick_scan_url() {
    let quick_type = dirk::ScanType::Quick;
    let urlbase: Uri = "http://127.0.0.1:3000".parse::<Uri>().unwrap();
    assert_eq!(
        quick_type.url(urlbase),
        "http://127.0.0.1:3000/scanner/quick".to_string()
    );
}

fn test_sigs() -> Vec<Signature> {
    let sig1 = Signature {
        action: Action::clean,
        comment: "".to_string(),
        date: 0,
        filenames: vec![],
        flat_string: false,
        id: "".to_string(),
        priority: Priority::high,
        severity: Severity::red,
        signature: "".to_string(),
        submitter: "fzylogic".to_string(),
        target: Target::Default,
    };
    let mut sigs = Vec::new();
    sigs.push(sig1);
    sigs
}

fn test_server(listener: TcpListener) -> Server<AddrIncoming, IntoMakeService<Router>> {
    let db = prepare_mock_db();
    let sigs = test_sigs();
    let app_state = Arc::new(DirkState { sigs, db });
    let scanner_app = dirk_api::build_router(app_state).expect("Unable to build router");
    axum::Server::from_tcp(listener)
        .expect("Unable to start server")
        .serve(scanner_app.into_make_service())
}

#[tokio::test]
async fn file_fetch() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Unable to bind to localhost");
    let port: u16 = listener.local_addr().unwrap().port();
    let server = test_server(listener);
    let _s = tokio::spawn(server);
    let client = reqwest::Client::new();

    let response = client
        .get(&format!("http://127.0.0.1:{}/files/2b998d019098754f1c0ae7eeb21fc4e673c6271b1d17593913ead73f5be772f1", port))
        .send()
        .await
        .expect("Failed to execute request.");
    assert!(response.status().is_success());
}

#[tokio::test]
async fn file_update() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Unable to bind to localhost");
    let port: u16 = listener.local_addr().unwrap().port();
    let server = test_server(listener);
    let _s = tokio::spawn(server);
    let client = reqwest::Client::new();

    let req = FileUpdateRequest {
        checksum: "acabee54d16c950ab5b694a296b41382f712c2d346a2a10b94e38ff8ea2d885b".to_string(),
        file_status: FileStatus::Good,
    };
    let response = client
        .post(&format!("http://127.0.0.1:{}/files", port))
        .json(&req)
        .send()
        .await
        .expect("Failed to execute request.");
    assert!(response.status().is_success());
}
