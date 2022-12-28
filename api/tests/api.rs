extern crate core;

mod prepare_db;

use axum::http::Uri;
use dirk_core::dirk_api;
use dirk_core::dirk_api::DirkState;
use dirk_core::hank::{Action, Priority, Severity, Signature, Target};
use prepare_db::prepare_mock_db;
use std::sync::Arc;
use std::net::TcpListener;

#[test]
fn full_scan_url() {
    let urlbase: Uri = "http://127.0.0.1:3000".parse::<Uri>().unwrap();
    let full_type = dirk_api::ScanType::Full;
    assert_eq!(
        full_type.url(urlbase),
        "http://127.0.0.1:3000/scanner/full".to_string()
    );
}

#[test]
fn quick_scan_url() {
    let quick_type = dirk_api::ScanType::Quick;
    let urlbase: Uri = "http://127.0.0.1:3000".parse::<Uri>().unwrap();
    assert_eq!(
        quick_type.url(urlbase),
        "http://127.0.0.1:3000/scanner/quick".to_string()
    );
}

#[tokio::test]
async fn health_check() {
    let db = prepare_mock_db();
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
    let app_state = Arc::new(DirkState { sigs, db });
    let scanner_app = dirk_api::build_router(app_state);
    let listener = TcpListener::bind("127.0.0.1:0")
        .expect("Unable to bind to localhost");
    let port: u16 = listener.local_addr().unwrap().port();
    let server = axum::Server::from_tcp(listener).expect("Unable to start server").serve(scanner_app.into_make_service());
    let _ = tokio::spawn(server);
    let client = reqwest::Client::new();

    let response = client
        .get(&format!("http://127.0.0.1:{}/health-check", port))
        .send()
        .await
        .expect("Failed to execute request.");
    assert!(response.status().is_success());
}
