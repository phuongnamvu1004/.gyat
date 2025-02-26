//! Simple wrapper around sha1 module.

use crate::Result;
use hex;
use sha1::{Digest, Sha1};
use std::{ffi::OsStr, fs, io::Read};

/// Digests the contents of a file into an SHA1 array.
///
/// # Parameters
/// * `file`: the file to digest.
/// # Returns
/// - `Ok` with the hashed array.
/// - `Err` if file reading fails.
pub fn digest_file(file: &mut fs::File) -> Result<[u8; 20]> {
    let mut buf: [u8; 1024] = [0; 1024];
    let mut len = file.read(&mut buf[..])?;
    let mut hasher = Sha1::new();
    while len > 0 {
        // if I don't qualify like this, there will be a conflict.
        hasher = sha1::digest::Update::chain(hasher, &buf[..]);
        buf = [0; 1024];
        len = file.read(&mut buf[..])?;
        // debug purpose. Comment out when running sha1_content_test
        // println!("{}", str::from_utf8(&buf).unwrap());
    }

    // todo!()
    Ok(hasher.finalize().into())
}

/// Generates the SHA1 in string form from the given content.
///
/// * `contents`: 
pub fn get_sha1_string(contents: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(contents);
    hex::encode(hasher.finalize())
}

/// Generates the SHA1 in bytes form from the given content.
///
/// * `content`: 
pub fn get_sha1_bytes(contents: &[u8]) -> [u8; 20] {
    let mut hasher = Sha1::new();
    hasher.update(contents);
    hasher.finalize().into()
}

#[inline]
/// Just a nicer name to `hex::encode(hash)`
///
/// * `hash`:
pub fn to_string(hash: &[u8; 20]) -> String {
    hex::encode(hash)
}

/// Convenience function to convert from a SHA1 string into a SHA1 array.
///
/// # Return value
/// - If the string cannot be converted to SHA1 bytes, return Err, otherwise Ok([u8; 20]).
/// * `s`:
pub fn from_string(s: &str) -> Result<[u8; 20]> {
    Ok(
        std::convert::TryInto::<[u8; 20]>::try_into(&hex::decode(s)?[..20])
            .or(Err(format!("Cannot convert {} into SHA1 bytes", s)))?,
    )
}

/// Convenience function to convert from a SHA1 OS string into a SHA1 array.
///
/// # Return value
/// - If the string cannot be converted to SHA1 bytes, return Err, otherwise Ok([u8; 20]).
///   - This function basically tries to convert an &OsStr into a &str (which it should be able to
///     since any OS should be able to display SHA1).
/// * `s`:
pub fn from_os_str(oss: &OsStr) -> Result<[u8; 20]> {
    // if it's "default", it's a fail right away.
    // I'm pretty sure any OS can represent a hex as a string.
    from_string(oss.to_str().unwrap_or_default())
}

#[cfg(test)]
mod test {
    use super::*;
    /// To ensure that 2 identical strings produce identical output.
    #[ignore = "This test is only to confirm SHA1's functionality"]
    #[test]
    fn sha1_test() {
        // good old placeholder text since the 1500s
        let test_str = "<!DOCTYPE html><head></head><body><p>Lorem ipsum dolor
            sit amet, consectetur adipiscing elit, sed do eiusmod tempor
            incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam,
            quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea
            commodo consequat. Duis aute irure dolor in reprehenderit in
            voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur
            sint occaecat cupidatat non proident, sunt in culpa qui officia
            deserunt mollit anim id est laborum.</p></body>";
        // also these are literally the same string. I don't know why I made 2 identifiers here.
        let test_str2 = "<!DOCTYPE html><head></head><body><p>Lorem ipsum dolor
            sit amet, consectetur adipiscing elit, sed do eiusmod tempor
            incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam,
            quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea
            commodo consequat. Duis aute irure dolor in reprehenderit in
            voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur
            sint occaecat cupidatat non proident, sunt in culpa qui officia
            deserunt mollit anim id est laborum.</p></body>";
        let mut hasher = Sha1::new();
        let a1 = {
            hasher.update(test_str);
            hasher.finalize_reset()
        };
        let a2 = {
            hasher.update(test_str);
            hasher.finalize_reset()
        };

        assert_eq!(a1, a2);
        let a1 = {
            hasher = sha1::digest::Update::chain(hasher, test_str);
            hasher = sha1::digest::Update::chain(hasher, test_str2);
            hasher.finalize_reset()
        };
        let a2 = {
            hasher = sha1::digest::Update::chain(hasher, test_str);
            hasher = sha1::digest::Update::chain(hasher, test_str2);
            hasher.finalize_reset()
        };
        assert_eq!(a1, a2);
    }

    /// 2 files with identical contents should return identical hashes.
    #[ignore = "This test is only to confirm SHA1's functionality"]
    #[test]
    fn sha1_file_test() {
        let a1 = digest_file(
            &mut fs::File::open("Cargo.toml").expect("Run the test at the project root!"),
        )
        .unwrap();
        let a2 = digest_file(&mut fs::File::open("test-data/cargo-mimic.txt").unwrap()).unwrap();

        assert_eq!(
            a1, a2,
            "Try run `cat Cargo.toml > test-data/cargo-mimic.txt` at the project root first\n
            If error still persists after that, this test fails."
        );
    }

    #[ignore = "Only run this test after un-commenting the println! inside digest_file"]
    #[test]
    fn sha1_content_test() {
        digest_file(&mut fs::File::open("src/hash.rs").unwrap()).unwrap();
    }
}
