//! Helper utility functions
use sha1::{Digest, Sha1};
use walkdir::DirEntry;

pub const MAX_FILESIZE: u64 = 2_000_000; // 2MB max file size to scan

/// Simple helper to return the String representation of the SHA1 checksum of a chunk of data
/// # Example
/// ```
/// use dirk_core::util;
///     let csum = util::checksum(&"dirk".as_bytes().to_vec());
///     assert_eq!(
///         csum,
///         "a00b27378a09822d5638cdfb8c2e7ccc36d74c56"
///     );
/// ```

pub fn checksum(data: &Vec<u8>) -> String {
    hex::encode(Sha1::digest(data))
}

/// Filter out files that are above our size threshold
pub fn filter_direntry(entry: &DirEntry) -> bool {
    if entry.path().is_file() {
        if let Ok(md) = entry.metadata() {
            if md.len() > MAX_FILESIZE {
                return false;
            }
        } else {
            eprintln!("Unable to fetch metadata for {}", entry.path().display());
            return false;
        }
    }
    true
}
