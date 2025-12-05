//! Contains all code dealing with system access.
use std::os::unix::fs::PermissionsExt;
use std::process::Child;
use std::{io::ErrorKind, process::Command};

use anyhow::anyhow;
use std::{
    env::{split_paths, var_os},
    fs::{read_dir, DirEntry},
    path::PathBuf,
};
use trie_rs::TrieBuilder;

use crate::error::EvalError;

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

        if let Ok(read_dir_iter) = read_dir(path) {
            for dir_entry in read_dir_iter.flatten() {
                if dir_entry.file_name() == file_name {
                    if let Ok(metadata) = dir_entry.metadata() {
                        let mode = metadata.permissions().mode();
                        if mode & 0o111 != 0 {
                            return Some(dir_entry);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Spawn a command and return the child process handle. This handles errors
/// in a way required by the challenge spec.
pub fn spawn_command(command: &mut Command) -> anyhow::Result<Child> {
    match command.spawn() {
        Ok(child) => Ok(child),
        Err(e) => {
            let message = match e.kind() {
                ErrorKind::NotFound => {
                    format!(
                        "{}: command not found",
                        command.get_program().to_string_lossy()
                    )
                }
                _ => format!("{}", e),
            };
            Err(EvalError::new(message))?
        }
    }
}

pub fn trie_builder_with_path_executables(paths: &[PathBuf]) -> TrieBuilder<u8> {
    let mut builder = TrieBuilder::new();

    for path in paths.iter() {
        if !path.exists() {
            continue;
        }

        if let Ok(read_dir_iter) = read_dir(path) {
            for dir_entry in read_dir_iter.flatten() {
                if let Ok(metadata) = dir_entry.metadata() {
                    let mode = metadata.permissions().mode();
                    if mode & 0o111 != 0 {
                        let file_name = dir_entry.file_name();
                        builder.push(file_name.as_encoded_bytes());
                    }
                }
            }
        }
    }

    builder
}
