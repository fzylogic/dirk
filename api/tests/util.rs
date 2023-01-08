use dirk_core::util;
#[test]
fn test_csum() {
    let csum = util::checksum(&"dirk".to_string());
    assert_eq!(
        csum,
        "2d69120f4a37384f5b712c447e7bd630eda348a5ad96ce3356900d6410935b56"
    );
}
