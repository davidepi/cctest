// This file is used to download the ANLTR parser generator (modified with rust support)
// and generate the different parsers for every grammar.
use curl::easy::Easy;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Error as IOError, ErrorKind, Read, Write};
use std::path::Path;

const ANTLR_PARSER_GENERATOR_URL: &str = "https://github.com/rrevenantt/antlr4rust/releases/download/antlr4-4.8-2-Rust0.3.0-beta/antlr4-4.8-2-SNAPSHOT-complete.jar";
const ANTLR_PARSER_GENERATOR_HASH: &str =
    "d23d7b0006f7477243d2d85c54632baa1932a5e05588e0c2548dbe3dd69f4637";
const RESOURCES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/resources");

fn main() {
    println!("cargo:rerun-if-changed=resources");
    download_file(
        ANTLR_PARSER_GENERATOR_URL,
        ANTLR_PARSER_GENERATOR_HASH,
        RESOURCES_DIR,
    )
    .expect("Failed to retrieve ANTLR parser generator");
}

/// Downloads a file from the web, and asserts the sha256 is the expected one.
fn download_file(
    url: &'static str,
    sha256: &'static str,
    dir: &'static str,
) -> Result<(), IOError> {
    // creates the folder if not existing, extract filename
    std::fs::create_dir_all(dir)?;
    let filename = Path::new(&url).file_name().unwrap().to_str().unwrap();
    let file_path = format!("{}/{}", dir, filename);
    let mut data = Vec::new();
    // try to read the file, if existing
    if let Ok(mut existing_file) = File::open(file_path.clone()) {
        if existing_file.read_to_end(&mut data).is_ok() {
            // check sha1 and return if correct, otherwise replace the file
            let mut hasher = Sha256::new();
            hasher.update(&data);
            let result = format!("{:x}", hasher.finalize());
            if result == sha256 {
                return Ok(());
            }
        }
    }
    // download new file
    data.clear();
    let mut easy = Easy::new();
    easy.url(url)?;
    easy.follow_location(true)?;
    {
        let mut transfer = easy.transfer();
        transfer.write_function(|c| {
            data.extend_from_slice(c);
            Ok(c.len())
        })?;
        transfer.perform()?;
    }
    // ensures the sha matches
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let result = format!("{:x}", hasher.finalize());
    if result == sha256 {
        let mut file = File::create(file_path)?;
        file.write_all(&data)
    } else {
        Err(IOError::new(
            ErrorKind::InvalidData,
            "Downloaded file with wrong sha256",
        ))
    }
}
