//! Contains all code dealing with system access.
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;

use std::{
    env::{split_paths, var_os},
    fs::{read_dir, DirEntry},
    path::PathBuf,
};

/// Changes the current directory.
pub fn change_directory(path: &PathBuf) -> Result<(), String> {
    match std::env::set_current_dir(path) {
        Ok(_) => Ok(()),
        Err(e) => {
            let message = match e.kind() {
                ErrorKind::NotFound => format!("cd: {}: No such file or directory", path.display()),
                _ => format!("{}", e),
            };
            Err(message)
        }
    }
}

/// Gets a vector of all paths in the PATH environment variable.
pub fn get_path() -> Vec<PathBuf> {
    match var_os("PATH") {
        Some(path) => split_paths(&path).collect(),
        None => {
            eprintln!("No PATH environment variable found!");
            Vec::new()
        }
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
                                    Err(e) => {
                                        eprintln!("error getting file type: {}", e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("error with dir entry: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("error reading dir {}: {}", path.display(), e);
            }
        }
    }
    None
}
