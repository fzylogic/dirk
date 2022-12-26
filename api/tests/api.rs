mod prepare_db;
use axum::http::Uri;
use dirk::dirk_api;
use dirk_core::dirk_api::DirkState;
use prepare_db::prepare_mock_db;

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
    let db = &prepare_mock_db();
    let state = DirkState { db, sigs };
}
