//! Helper utility functions
use sha1::{Digest, Sha1};

/// Simple helper to return the String representation of the SHA1 checksum of a chunk of data
/// # Example
/// ```
/// use dirk_core::util;
///     let csum = util::checksum(&"dirk".to_string());
///     assert_eq!(
///         csum,
///         "a00b27378a09822d5638cdfb8c2e7ccc36d74c56"
///     );
/// ```

pub fn checksum(data: &String) -> String {
    hex::encode(Sha1::digest(data.as_bytes()))
}
