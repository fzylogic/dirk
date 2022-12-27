extern crate core;

mod prepare_db;

use axum::http::Uri;
use dirk_core::dirk_api;
use dirk_core::dirk_api::DirkState;
use dirk_core::hank::{Action, Priority, Severity, Signature, Target};
use prepare_db::prepare_mock_db;
use std::path::PathBuf;
use std::sync::Arc;

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

#[test]
fn health_check() {
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
    let scanner_app = build_router(app_state);
    let server = axum::Server::bind(&addr).serve(scanner_app.into_make_service());
    let _ = tokio::spawn(server);
}
