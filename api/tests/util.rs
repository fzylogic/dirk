use dirk_core::util;
#[test]
fn test_csum() {
    let csum = util::checksum(&"dirk".as_bytes().to_vec());
    assert_eq!(csum, "a00b27378a09822d5638cdfb8c2e7ccc36d74c56");
}
