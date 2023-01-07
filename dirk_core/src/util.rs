//! Helper utility functions

use sha256::digest;

/// Simple helper to return the String representation of the SHA256 checksum of a chunk of data
/// # Example
/// ```
/// use dirk_core::util;
///     let csum = util::checksum(&"dirk".to_string());
///     assert_eq!(
///         csum,
///         "2d69120f4a37384f5b712c447e7bd630eda348a5ad96ce3356900d6410935b56"
///     );
/// ```

pub fn checksum(data: &String) -> String {
    digest(data.to_string())
}
