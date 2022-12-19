use dirk::util;

#[test]
fn sha256sum() {
    let csum = util::checksum(&"dirk".to_string());
    assert_eq!(csum, "2d69120f4a37384f5b712c447e7bd630eda348a5ad96ce3356900d6410935b56");
}

/*#[test]
fn deserialize_bool() {

}*/