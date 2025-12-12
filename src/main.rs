mod ast;
mod editing;
mod error;
mod parser;
mod scanner;
mod system;

use crate::ast::*;
use crate::editing::*;
use crate::parser::*;
use crate::system::*;
use rustyline::history::{History, SearchDirection};
use std::fs::File;
use std::fs::OpenOptions;
use std::io::{self, Cursor, Write};
use std::path::PathBuf;
use std::process::{Child, Stdio};

fn main() -> anyhow::Result<()> {
    let paths = get_path();
    let mut editor = create_editor(&paths)?;
    loop {
        let command_text = editor.readline("$ ")?;
        if let Err(e) = eval(&paths, editor.history(), &command_text) {
            eprintln!("{}", e);
        }
    }
}

fn eval<H>(paths: &[PathBuf], history: &H, command_text: &str) -> anyhow::Result<()>
where
    H: History,
{
    let pipeline = parse(command_text)?;
    let n = pipeline.len();

    // This has the child process for each item command in the pipeline. If the
    // command was a built-in then `None` is pushed.
    let mut children = Vec::<Option<Child>>::new();

    // If the previous command was a built-in, this holds its output buffer.
    let mut built_in_out = None;

    for (i, command) in pipeline.iter().enumerate() {
        let is_first = i == 0;
        let is_last = i + 1 == n;

        match command {
            Command::BuiltIn(command) => {
                // If there is any output for previous command in pipeline
                // we should discard it.
                let _ = built_in_out.take();

                let out = eval_built_in_command(paths, history, command)?;
                if is_last {
                    io::stdout().write_all(&out)?;
                } else {
                    built_in_out.replace(out);
                }

                // Built-ins don't create child processes.
                children.push(None);
            }

            Command::External(command) => {
                let stdin = if is_first {
                    Stdio::inherit()
                } else if let Some(last_child) = &mut children[i - 1] {
                    last_child
                        .stdout
                        .take()
                        .map(Stdio::from)
                        .unwrap_or_else(Stdio::piped)
                } else {
                    Stdio::piped()
                };

                let stdout = if is_last {
                    Stdio::inherit()
                } else {
                    Stdio::piped()
                };

                let mut command = eval_external_command(command, stdin, stdout)?;
                let mut child = spawn_command(&mut command)?;

                if let Some(buf) = built_in_out.take() {
                    if let Some(mut stdin) = child.stdin.take() {
                        stdin.write_all(&buf)?;
                        stdin.flush()?;
                    }
                }

                children.push(Some(child));
            }
        }
    }

    for child in children.iter_mut().flatten() {
        child.wait()?;
    }

    Ok(())
}

/// Evaluates a built in command. Returns stdout contents, if any.
fn eval_built_in_command<H>(
    paths: &[PathBuf],
    history: &H,
    built_in_command: &BuiltInCommand,
) -> anyhow::Result<Vec<u8>>
where
    H: History,
{
    match &built_in_command.redirection {
        Redirection::StdOut {
            filename,
            is_append,
        } => {
            let mut stdout = open_file(filename, *is_append)?;
            let mut stderr = io::stderr();
            eval_built_in(
                paths,
                history,
                &mut stdout,
                &mut stderr,
                &built_in_command.built_in,
            )?;
            Ok(Vec::new())
        }

        Redirection::StdErr {
            filename,
            is_append,
        } => {
            let mut stdout = Cursor::new(Vec::new());
            let mut stderr = open_file(filename, *is_append)?;
            eval_built_in(
                paths,
                history,
                &mut stdout,
                &mut stderr,
                &built_in_command.built_in,
            )?;
            Ok(stdout.into_inner())
        }

        Redirection::None => {
            let mut stdout = Cursor::new(Vec::new());
            let mut stderr = io::stderr();
            eval_built_in(
                paths,
                history,
                &mut stdout,
                &mut stderr,
                &built_in_command.built_in,
            )?;
            Ok(stdout.into_inner())
        }
    }
}

/// Evaluates a built in command.
fn eval_built_in<H, TOut: Write, TErr: Write>(
    paths: &[PathBuf],
    history: &H,
    stdout: &mut TOut,
    stderr: &mut TErr,
    built_in: &BuiltIn,
) -> anyhow::Result<()>
where
    H: History,
{
    match built_in {
        BuiltIn::Echo(args) => {
            if !args.is_empty() {
                write!(stdout, "{}", args[0])?;
                for arg in args.iter().skip(1) {
                    write!(stdout, " {}", arg)?;
                }
            }
            writeln!(stdout)?;
        }
        BuiltIn::Cd(path) if path == "~" => match std::env::home_dir() {
            Some(home) => change_directory(&home)?,
            None => writeln!(stderr, "cd: Home directory is unknown")?,
        },
        BuiltIn::Cd(path) => {
            if let Err(e) = change_directory(&PathBuf::from(path)) {
                writeln!(stderr, "cd: {e}")?;
            }
        }
        BuiltIn::Exit(code) => {
            std::process::exit(*code);
        }
        BuiltIn::Pwd => match std::env::current_dir() {
            Ok(current_dir) => {
                writeln!(stdout, "{}", current_dir.display())?;
            }
            Err(e) => {
                writeln!(stderr, "{}", e)?;
            }
        },
        BuiltIn::Type(command) => match command.as_ref() {
            "cd" | "echo" | "exit" | "history" | "pwd" | "type" => {
                writeln!(stdout, "{} is a shell builtin", command)?;
            }
            _ => match search_for_executable_file(paths, command) {
                Some(dir_entry) => {
                    writeln!(stdout, "{} is {}", command, dir_entry.path().display())?;
                }
                None => {
                    writeln!(stderr, "{}: not found", command)?;
                }
            },
        },
        BuiltIn::History(limit) => {
            print_history(history, stdout, limit)?;
        }
    }
    Ok(())
}

fn print_history<H: History, TOut: Write>(
    history: &H,
    stdout: &mut TOut,
    limit: &Option<usize>,
) -> anyhow::Result<()> {
    let len = history.len();
    let start = match limit {
        Some(limit) if *limit >= len => 0,
        Some(limit) => len - limit,
        None => 0,
    };
    for i in start..len {
        if let Some(result) = history.get(i, SearchDirection::Forward)? {
            writeln!(stdout, "\t{}\t{}", i + 1, result.entry)?;
        }
    }
    Ok(())
}

fn eval_external_command(
    external_command: &ExternalCommand,
    stdin: Stdio,
    stdout: Stdio,
) -> anyhow::Result<std::process::Command> {
    match &external_command.redirection {
        Redirection::StdOut {
            filename,
            is_append,
        } => {
            let stdout = Stdio::from(open_file(filename, *is_append)?);
            let stderr = Stdio::inherit();
            let command = eval_external(&external_command.args, stdin, stdout, stderr)?;
            Ok(command)
        }

        Redirection::StdErr {
            filename,
            is_append,
        } => {
            let stderr = Stdio::from(open_file(filename, *is_append)?);
            let command = eval_external(&external_command.args, stdin, stdout, stderr)?;
            Ok(command)
        }

        Redirection::None => {
            let stderr = Stdio::inherit();
            let command = eval_external(&external_command.args, stdin, stdout, stderr)?;
            Ok(command)
        }
    }
}

/// Evaluates an external command, e.g. `cd`.
fn eval_external(
    args: &[String],
    stdin: Stdio,
    stdio: Stdio,
    stderr: Stdio,
) -> anyhow::Result<std::process::Command> {
    assert!(!args.is_empty());
    let command_name = &args[0];
    let args = args.iter().skip(1);
    let mut command = std::process::Command::new(command_name);
    command.args(args).stdin(stdin).stdout(stdio).stderr(stderr);
    Ok(command)
}

/// Creates a file.
fn open_file(filename: &str, is_append: bool) -> io::Result<File> {
    let mut open_options = OpenOptions::new();

    if is_append {
        open_options.append(true);
    } else {
        open_options.truncate(true);
    }

    open_options.write(true).create(true).open(filename)
}
