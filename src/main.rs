mod ast;
mod error;
mod parser;
mod scanner;
mod system;

use crate::ast::*;
use crate::error::EvalError;
use crate::parser::*;
use crate::system::change_directory;
use crate::system::get_path;
use crate::system::search_for_executable_file;
use std::fs::File;
use std::io::ErrorKind;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Stdio;

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
fn read_eval(paths: &[PathBuf]) -> anyhow::Result<()> {
    print!("$ ");
    let command_text = read()?;
    eval(paths, &command_text)
}

/// Reads a command.
fn read() -> anyhow::Result<String> {
    io::stdout().flush()?;

    let mut input = String::new();
    _ = io::stdin().read_line(&mut input)?;
    Ok(input)
}

/// Parses and evaluates a command.
fn eval(paths: &[PathBuf], command_text: &str) -> anyhow::Result<()> {
    let Some(command) = parse(command_text)? else {
        return Ok(());
    };
    eval_command(paths, command)
}

/// Evaluates a command.
fn eval_command(paths: &[PathBuf], command: Command) -> anyhow::Result<()> {
    match command.simple_command {
        SimpleCommand::BuiltIn(built_in) => {
            let mut stdio: Box<dyn Write> = match command.redirection {
                None => Box::new(io::stdout()),
                Some(Redirection::Output {
                    file_descriptor: 1,
                    target,
                }) => Box::new(File::create(target)?),
                Some(Redirection::Output {
                    file_descriptor, ..
                }) => {
                    let message = format!("unrecognized file descriptor `{file_descriptor}`");
                    Err(EvalError::new(message))?
                }
            };
            eval_built_in(paths, &mut stdio, built_in)
        }
        SimpleCommand::External(args) => {
            let stdio = match command.redirection {
                None => Stdio::inherit(),
                Some(Redirection::Output {
                    file_descriptor: 1,
                    target,
                }) => {
                    let file = File::create(target)?;
                    Stdio::from(file)
                }
                Some(Redirection::Output {
                    file_descriptor, ..
                }) => {
                    let message = format!("unrecognized file descriptor `{file_descriptor}`");
                    Err(EvalError::new(message))?
                }
            };
            eval_external(args, stdio)
        }
    }
}

/// Evaluates a built in command.
fn eval_built_in(
    paths: &[PathBuf],
    stdio: &mut Box<dyn Write>,
    built_in: BuiltIn,
) -> anyhow::Result<()> {
    match built_in {
        BuiltIn::Echo(args) => {
            if !args.is_empty() {
                write!(stdio, "{}", args[0])?;
                for arg in args.iter().skip(1) {
                    write!(stdio, " {}", arg)?;
                }
            }
            writeln!(stdio)?;
            Ok(())
        }
        BuiltIn::Cd(path) if path == "~" => match std::env::home_dir() {
            Some(home) => change_directory(&home),
            None => Err(EvalError::from_str("cd: Home directory is unknown"))?,
        },
        BuiltIn::Cd(path) => change_directory(&PathBuf::from(path)),
        BuiltIn::Exit(code) => {
            std::process::exit(code);
        }
        BuiltIn::Pwd => match std::env::current_dir() {
            Ok(current_dir) => {
                writeln!(stdio, "{}", current_dir.display())?;
                Ok(())
            }
            Err(e) => {
                let message = format!("{}", e);
                Err(EvalError::new(message))?
            }
        },
        BuiltIn::Type(command) => match command.as_ref() {
            "cd" | "echo" | "exit" | "pwd" | "type" => {
                writeln!(stdio, "{} is a shell builtin", command)?;
                Ok(())
            }
            _ => match search_for_executable_file(paths, &command) {
                Some(dir_entry) => {
                    writeln!(stdio, "{} is {}", command, dir_entry.path().display())?;
                    Ok(())
                }
                None => {
                    let message = format!("{}: not found", command);
                    Err(EvalError::new(message))?
                }
            },
        },
    }
}

/// Evaluates an external command, e.g. `cd`.
fn eval_external(args: Vec<String>, stdio: Stdio) -> anyhow::Result<()> {
    assert!(!args.is_empty());
    let command = &args[0];
    let args = args.iter().skip(1);
    let status = std::process::Command::new(command)
        .args(args)
        .stdout(stdio)
        .status();
    match status {
        Ok(_) => Ok(()),
        Err(e) => {
            let message = match e.kind() {
                ErrorKind::NotFound => format!("{}: command not found", command),
                _ => format!("{}", e),
            };
            Err(EvalError::new(message))?
        }
    }
}
