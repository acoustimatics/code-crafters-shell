mod ast;
mod parser;
mod scanner;
mod system;

use crate::ast::*;
use crate::parser::*;
use crate::system::change_directory;
use crate::system::get_path;
use crate::system::search_for_executable_file;
use std::io::ErrorKind;
use std::io::{self, Write};
use std::path::PathBuf;

fn main() {
    let paths = get_path();
    repl(&paths);
}

/// Read, eval, print loop.
fn repl(paths: &[PathBuf]) -> ! {
    loop {
        match read_eval(paths) {
            Ok(_) => {}
            Err(e) => eprintln!("{}", e),
        }
    }
}

/// Reads and evaluates a command.
fn read_eval(paths: &[PathBuf]) -> Result<(), String> {
    print!("$ ");
    let command_text = read()?;
    eval(paths, &command_text)?;
    Ok(())
}

/// Reads a command.
fn read() -> Result<String, String> {
    match io::stdout().flush() {
        Ok(_) => {}
        Err(e) => return Err(format!("{}", e)),
    }

    let mut input = String::new();
    match io::stdin().read_line(&mut input) {
        Ok(_) => Ok(input),
        Err(e) => return Err(format!("{}", e)),
    }
}

/// Evaluates a command.
fn eval(paths: &[PathBuf], command_text: &str) -> Result<(), String> {
    match parse(command_text)? {
        Command::Empty => Ok(()),
        Command::Echo(args) => {
            if args.len() > 0 {
                print!("{}", args[0]);
                for arg in args.iter().skip(1) {
                    print!(" {}", arg);
                }
            }
            println!("");
            Ok(())
        }
        Command::Cd(path) if path == "~" => match std::env::home_dir() {
            Some(home) => change_directory(&home),
            None => Err(String::from("cd: Home directory is unknown")),
        },
        Command::Cd(path) => change_directory(&PathBuf::from(path)),
        Command::Exit(code) => {
            std::process::exit(code);
        }
        Command::External(args) => {
            assert!(args.len() > 0);
            let command = &args[0];
            let args = args.iter().skip(1);
            let status = std::process::Command::new(command).args(args).status();
            match status {
                Ok(_) => Ok(()),
                Err(e) => {
                    let message = match e.kind() {
                        ErrorKind::NotFound => format!("{}: command not found", command),
                        _ => format!("{}", e),
                    };
                    Err(message)
                }
            }
        }
        Command::Pwd => match std::env::current_dir() {
            Ok(current_dir) => {
                println!("{}", current_dir.display());
                Ok(())
            }
            Err(e) => {
                let message = format!("{}", e);
                Err(message)
            }
        },
        Command::Type(command) => match command.as_ref() {
            "cd" | "echo" | "exit" | "pwd" | "type" => {
                println!("{} is a shell builtin", command);
                Ok(())
            }
            _ => match search_for_executable_file(paths, &command) {
                Some(dir_entry) => {
                    println!("{} is {}", command, dir_entry.path().display());
                    Ok(())
                }
                None => {
                    let message = format!("{}: not found", command);
                    Err(message)
                }
            },
        },
    }
}
