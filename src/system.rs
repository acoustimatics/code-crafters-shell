//! Contains all code dealing with system access.
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;

use anyhow::anyhow;
use trie_rs::TrieBuilder;
use std::{
    env::{split_paths, var_os},
    fs::{read_dir, DirEntry},
    path::PathBuf,
};

/// Changes the current directory.
pub fn change_directory(path: &PathBuf) -> anyhow::Result<()> {
    match std::env::set_current_dir(path) {
        Ok(_) => Ok(()),
        Err(e) => {
            if let ErrorKind::NotFound = e.kind() {
                Err(anyhow!("{}: No such file or directory", path.display()))?
            }
            Err(e)?
        }
    }
}

/// Gets a vector of all paths in the PATH environment variable.
pub fn get_path() -> Vec<PathBuf> {
    match var_os("PATH") {
        Some(path) => split_paths(&path).collect(),
        None => Vec::new(),
    }
}

/// Searches for an executable file in a collection of paths.
pub fn search_for_executable_file(paths: &[PathBuf], file_name: &str) -> Option<DirEntry> {
    for path in paths.iter() {
        if !path.exists() {
            continue;
        }

        match read_dir(path) {
            Ok(read_dir_iter) => {
                for dir_entry in read_dir_iter {
                    match dir_entry {
                        Ok(dir_entry) => {
                            if dir_entry.file_name() == file_name {
                                match dir_entry.metadata() {
                                    Ok(metadata) => {
                                        let mode = metadata.permissions().mode();
                                        if mode & 0o111 != 0 {
                                            return Some(dir_entry);
                                        }
                                    }
                                    Err(_) => {}
                                }
                            }
                        }
                        Err(_) => {}
                    }
                }
            }
            Err(_) => {}
        }
    }
    None
}

pub fn trie_builder_with_path_executables(paths: &[PathBuf]) -> TrieBuilder<u8> {
    let mut builder = TrieBuilder::new();

    for path in paths.iter() {
        if !path.exists() {
            continue;
        }

        match read_dir(path) {
            Ok(read_dir_iter) => {
                for dir_entry in read_dir_iter {
                    match dir_entry {
                        Ok(dir_entry) => {
                            match dir_entry.metadata() {
                                Ok(metadata) => {
                                    let mode = metadata.permissions().mode();
                                    if mode & 0o111 != 0 {
                                        let file_name = dir_entry.file_name();
                                        builder.push(file_name.as_encoded_bytes());
                                    }
                                }
                                Err(_) => {}
                            }
                        }
                        Err(_) => {}
                    }
                }
            }
            Err(_) => {}
        }
    }
    
    builder
}
