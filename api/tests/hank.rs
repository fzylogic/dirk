use dirk_core::models::*;
#[test]
fn test_resultstatus_display() {
    assert_eq!(hank::ResultStatus::OK.to_string(), "OK".to_string());
    assert_eq!(hank::ResultStatus::Bad.to_string(), "BAD".to_string());
    assert_eq!(hank::ResultStatus::Inconclusive.to_string(), "Inconclusive".to_string());
}