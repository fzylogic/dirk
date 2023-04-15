use dirk_core::models::*;
#[test]
fn test_resultstatus_display() {
    assert_eq!(yara::ResultStatus::OK.to_string(), "OK".to_string());
    assert_eq!(yara::ResultStatus::Bad.to_string(), "BAD".to_string());
    assert_eq!(
        yara::ResultStatus::Inconclusive.to_string(),
        "Inconclusive".to_string()
    );
}
