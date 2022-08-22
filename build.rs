//
// This file is used to download the ANLTR parser generator (modified with rust support)
// and generate the different parsers for every grammar.
//

use curl::easy::Easy;
use serde_derive::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{Error as IOError, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const ANTLR_PARSER_GENERATOR_URL: &str = "https://github.com/rrevenantt/antlr4rust/releases/download/antlr4-4.8-2-Rust0.3.0-beta/antlr4-4.8-2-SNAPSHOT-complete.jar";
const ANTLR_PARSER_GENERATOR_HASH: &str =
    "d23d7b0006f7477243d2d85c54632baa1932a5e05588e0c2548dbe3dd69f4637";

#[derive(Deserialize)]
struct GrammarDownload {
    url: Vec<String>,
    sha256: Vec<String>,
}

fn main() -> Result<(), IOError> {
    println!("cargo:rerun-if-changed=grammars/grammars.toml");
    let outdir = env::var_os("OUT_DIR")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let antlr_dir = Path::new(&outdir).join("antlr");
    let downloaded_dir = Path::new(&outdir).join("downloaded");
    let generated_dir = Path::new(&outdir).join("generated");
    std::fs::create_dir_all(&antlr_dir)?;
    std::fs::create_dir_all(&downloaded_dir)?;
    std::fs::create_dir_all(&generated_dir)?;
    download_file(
        ANTLR_PARSER_GENERATOR_URL,
        ANTLR_PARSER_GENERATOR_HASH,
        &antlr_dir,
    )?;
    download_grammars("grammars/grammars.toml", &downloaded_dir)?;
    run_parser_generator(antlr_dir, downloaded_dir, generated_dir)?;
    Ok(())
}

/// Reads the content of a file (`list`) and downloads the grammars listed there
fn download_grammars<P: AsRef<Path>>(list: &str, outdir: P) -> Result<(), IOError> {
    let list_content = std::fs::read_to_string(list)?;
    let toml: HashMap<String, GrammarDownload> = toml::from_str(&list_content)?;
    for (key, value) in toml {
        assert_eq!(
            value.url.len(),
            value.sha256.len(),
            "The amount of URLs and SHA-256 for {} is different",
            key
        );
        let langdir = PathBuf::from(outdir.as_ref()).join(key);
        std::fs::create_dir_all(&langdir)?;
        for (url, sha256) in value.url.iter().zip(value.sha256.iter()) {
            download_file(url, sha256, &langdir)?;
        }
    }
    Ok(())
}

/// Runs the ANTLR parser generator for each grammar inside downloaded_dir/<lang>/<files.g4> and
/// generates the Rust target at generated_dir/<lang>/<files>
fn run_parser_generator<P: AsRef<Path>>(
    antlr_dir: P,
    downloaded_dir: P,
    generated_dir: P,
) -> Result<(), IOError> {
    let antlr_filename = Path::new(&ANTLR_PARSER_GENERATOR_URL)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    let antlr_path = PathBuf::from(antlr_dir.as_ref()).join(antlr_filename);
    let entries = std::fs::read_dir(downloaded_dir)?;
    for entry in entries {
        let path = entry?.path(); // i.e. downloaded_dir/java
        if path.is_dir() {
            let outdir = PathBuf::from(generated_dir.as_ref()).join(path.file_name().unwrap());
            std::fs::create_dir_all(&outdir)?;
            let grammar_entries = std::fs::read_dir(path)?;
            for grammar_entry in grammar_entries {
                let grammar_path = grammar_entry?.path(); // i.e. downloaded_dir/java/Lexer.g4
                if grammar_path.is_file() {
                    if let Some(extension) = grammar_path.extension() {
                        if extension.to_str().unwrap() == "g4" {
                            Command::new("java")
                                .arg("-cp")
                                .arg(&antlr_path)
                                .arg("org.antlr.v4.Tool")
                                .arg("-Dlanguage=Rust")
                                .arg("-o")
                                .arg(&outdir)
                                .arg(grammar_path)
                                .spawn()?
                                .wait_with_output()?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Downloads a file from the web, and asserts the sha256 is the expected one.
fn download_file<P: AsRef<Path>>(url: &str, sha256: &str, dir: P) -> Result<(), IOError> {
    // creates the folder if not existing, extract filename
    std::fs::create_dir_all(&dir)?;
    let filename = Path::new(&url).file_name().unwrap().to_str().unwrap();
    let file_path = PathBuf::from(dir.as_ref()).join(filename);
    let mut data = Vec::new();
    // try to read the file, if existing
    if let Ok(mut existing_file) = File::open(&file_path) {
        if existing_file.read_to_end(&mut data).is_ok() {
            // check sha1 and return if correct, otherwise replace the file
            let mut hasher = Sha256::new();
            hasher.update(&data);
            let result = format!("{:x}", hasher.finalize());
            if result == sha256 {
                println!("Skipping existing file {}", file_path.display());
                return Ok(());
            }
        }
    }
    println!("Downloading to {}", file_path.display());
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
            "Downloaded file with wrong SHA-256",
        ))
    }
}
