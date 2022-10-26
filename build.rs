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
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use wisent::error::ParseError;
use wisent::grammar::Grammar;
use wisent::lexer::Dfa;

#[derive(Deserialize)]
struct GrammarDownload {
    url: String,
    sha256: String,
    extensions: Vec<String>,
}

fn main() -> Result<(), BuildScriptError> {
    println!("cargo:rerun-if-changed=grammars.toml");
    let outdir = env::var_os("OUT_DIR")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let downloaded_dir = Path::new(&outdir).join("downloaded");
    let generated_dir = Path::new(&outdir).join("generated");
    let output_rust_file = Path::new(&outdir).join("assign_grammars.in");
    std::fs::create_dir_all(&downloaded_dir)?;
    std::fs::create_dir_all(&generated_dir)?;
    let dfa_map = download_and_generate_grammars("grammars.toml", &downloaded_dir, &generated_dir)?;
    print_grammar_assignment(dfa_map, output_rust_file)?;
    Ok(())
}

/// Prints a file matching the extensions to the generated grammar
fn print_grammar_assignment<P: AsRef<Path>>(
    dfa_map: HashMap<String, String>,
    output: P,
) -> Result<(), BuildScriptError> {
    let mut f = File::create(output)?;
    writeln!(
        f,
        "/// Returns a vector of bytes containing the lexer DFA implementation, given"
    )?;
    writeln!(f, "/// the file extension.")?;
    writeln!(f, "fn assign_dfas(extension: &str) {{")?;
    writeln!(f, "    match extension {{")?;
    for (extension, dfa_bytes) in dfa_map.into_iter() {
        if extension.chars().all(|x| x.is_alphanumeric()) {
            writeln!(
                f,
                "        \"{}\" => include_bytes!(\"{}\"),",
                extension, dfa_bytes
            )?;
        } else {
            Err(std::io::Error::new(
                ErrorKind::InvalidData,
                format!("non alphanumeric extension '{}'", extension),
            ))?
        }
    }
    writeln!(f, "        _ => Vec::new(),")?;
    write!(f, "    }}\n}}\n")?;
    Ok(())
}

/// Reads the content of a file (`list`) and downloads the grammars listed there
/// Then runs the parser generator for each of them
/// returns the pair (extension, parser_to_use)
fn download_and_generate_grammars<P: AsRef<Path>>(
    list: &str,
    downloaded_dir: P,
    generated_dir: P,
) -> Result<HashMap<String, String>, BuildScriptError> {
    let list_content = std::fs::read_to_string(list)?;
    let toml: HashMap<String, GrammarDownload> = toml::from_str(&list_content)?;
    let mut parsers = HashMap::new();
    for (key, value) in toml {
        let downloaded_langdir = PathBuf::from(downloaded_dir.as_ref()).join(&key);
        let generated_langdir = PathBuf::from(generated_dir.as_ref()).join(&key);
        std::fs::create_dir_all(&generated_langdir)?;
        download_file(&value.url, &value.sha256, &downloaded_langdir)?;
        let filename = Path::new(&value.url).file_name().unwrap().to_str().unwrap();
        let filestem = Path::new(&value.url).file_stem().unwrap().to_str().unwrap();
        let downloaded_file = downloaded_langdir.join(filename);
        let generated_file = generated_langdir.join(format!("{}.dfa", filestem));
        let grammar = Grammar::parse_grammar(downloaded_file.as_path().to_str().unwrap())?;
        let dfa = Dfa::new(&grammar);
        let encoded_dfa = dfa.as_bytes();
        std::fs::write(generated_file.as_path(), encoded_dfa)?;
        for extension in value.extensions {
            parsers.insert(
                extension,
                generated_file.as_path().to_str().unwrap().to_string(),
            );
        }
    }
    Ok(parsers)
}

/// Downloads a file from the web, and asserts the sha256 is the expected one.
fn download_file<P: AsRef<Path>>(url: &str, sha256: &str, dir: P) -> Result<(), std::io::Error> {
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
        Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "Downloaded file with wrong SHA-256",
        ))
    }
}

#[derive(Debug)]
enum BuildScriptError {
    Parse(ParseError),
    Toml(toml::de::Error),
    Io(std::io::Error),
}

impl From<ParseError> for BuildScriptError {
    fn from(err: ParseError) -> Self {
        BuildScriptError::Parse(err)
    }
}

impl From<std::io::Error> for BuildScriptError {
    fn from(err: std::io::Error) -> Self {
        BuildScriptError::Io(err)
    }
}

impl From<toml::de::Error> for BuildScriptError {
    fn from(err: toml::de::Error) -> Self {
        BuildScriptError::Toml(err)
    }
}
