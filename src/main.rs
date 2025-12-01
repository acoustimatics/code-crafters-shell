mod ast;
mod error;
mod parser;
mod scanner;
mod system;

use rustyline::completion::Candidate;
use rustyline::Helper;
use rustyline::{
    Completer, CompletionType, Config, Context, Editor, Highlighter, Hinter, Validator,
};

use crate::ast::*;
use crate::error::EvalError;
use crate::parser::*;
use crate::system::get_path;
use crate::system::search_for_executable_file;
use crate::system::{change_directory, trie_builder_with_path_executables};
use std::fs::File;
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Child, Stdio};

fn main() -> anyhow::Result<()> {
    let paths = get_path();

    let completer = ShellCompleter::new(&paths);

    let helper = ShellHelper::new(completer);

    let mut editor = {
        let config = Config::builder()
            .completion_type(CompletionType::List)
            .build();
        let mut editor = Editor::with_config(config)?;
        editor.set_helper(Some(helper));
        editor
    };

    loop {
        let command_text = editor.readline("$ ")?;
        if let Err(e) = eval(&paths, &command_text) {
            eprintln!("{}", e);
        }
    }
}

/// Parses and evaluates a command.
fn eval(paths: &[PathBuf], command_text: &str) -> anyhow::Result<()> {
    let Some(Pipeline::Single(command)) = parse(command_text)? else {
        return Ok(());
    };
    if let Some(mut child) = eval_command(paths, command)? {
        let _status = child.wait()?;
    }
    Ok(())
}

/// Evaluates a command.
fn eval_command(paths: &[PathBuf], command: Command) -> anyhow::Result<Option<Child>> {
    use Command::*;

    match command {
        BuiltIn {
            built_in,
            redirection: Redirection::StdOut {
                filename,
                is_append,
            },
        } => {
            let mut stdout = open_file(filename, is_append)?;
            let mut stderr = io::stderr();
            eval_built_in(paths, &mut stdout, &mut stderr, built_in)?;
            Ok(None)
        }

        BuiltIn {
            built_in,
            redirection: Redirection::StdErr {
                filename,
                is_append,
            },
        } => {
            let mut stdout = io::stdout();
            let mut stderr = open_file(filename, is_append)?;
            eval_built_in(paths, &mut stdout, &mut stderr, built_in)?;
            Ok(None)
        }

        BuiltIn {
            built_in,
            redirection: Redirection::None,
        } => {
            let mut stdout = io::stdout();
            let mut stderr = io::stderr();
            eval_built_in(paths, &mut stdout, &mut stderr, built_in)?;
            Ok(None)
        }

        External {
            args,
            redirection: Redirection::StdOut {
                filename,
                is_append,
            },
        } => {
            let stdio = Stdio::from(open_file(filename, is_append)?);
            let stderr = Stdio::inherit();
            let child = eval_external(args, stdio, stderr)?;
            Ok(Some(child))
        }

        External {
            args,
            redirection: Redirection::StdErr {
                filename,
                is_append,
            },
        } => {
            let stdio = Stdio::inherit();
            let stderr = Stdio::from(open_file(filename, is_append)?);
            let child = eval_external(args, stdio, stderr)?;
            Ok(Some(child))
        }

        External {
            args,
            redirection: Redirection::None,
        } => {
            let stdio = Stdio::inherit();
            let stderr = Stdio::inherit();
            let child = eval_external(args, stdio, stderr)?;
            Ok(Some(child))
        }
    }
}

/// Evaluates a built in command.
fn eval_built_in<TOut: Write, TErr: Write>(
    paths: &[PathBuf],
    stdout: &mut TOut,
    stderr: &mut TErr,
    built_in: BuiltIn,
) -> anyhow::Result<()> {
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
            std::process::exit(code);
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
                "cd" | "echo" | "exit" | "pwd" | "type" => {
                    writeln!(stdout, "{} is a shell builtin", command)?;
                }
                _ => match search_for_executable_file(paths, &command) {
                    Some(dir_entry) => {
                        writeln!(stdout, "{} is {}", command, dir_entry.path().display())?;
                    }
                    None => {
                        writeln!(stderr, "{}: not found", command)?;
                    }
                },
        },
    }
    Ok(())
}

/// Evaluates an external command, e.g. `cd`.
fn eval_external(args: Vec<String>, stdio: Stdio, stderr: Stdio) -> anyhow::Result<Child> {
    assert!(!args.is_empty());
    let command = &args[0];
    let args = args.iter().skip(1);
    let child = std::process::Command::new(command)
        .args(args)
        .stdout(stdio)
        .stderr(stderr)
        .spawn();
    match child {
        Ok(child) => Ok(child),
        Err(e) => {
            let message = match e.kind() {
                ErrorKind::NotFound => format!("{}: command not found", command),
                _ => format!("{}", e),
            };
            Err(EvalError::new(message))?
        }
    }
}

/// Creates a file.
fn open_file(filename: String, is_append: bool) -> io::Result<File> {
    let mut open_options = OpenOptions::new();

    if is_append {
        open_options.append(true);
    } else {
        open_options.truncate(true);
    }

    open_options.write(true).create(true).open(filename)
}

#[derive(Helper, Completer, Hinter, Highlighter, Validator)]
struct ShellHelper<'a> {
    #[rustyline(Completer)]
    completer: ShellCompleter<'a>,
}

impl<'a> ShellHelper<'a> {
    fn new(completer: ShellCompleter<'a>) -> Self {
        Self { completer }
    }
}

struct ShellCompleter<'a> {
    paths: &'a [PathBuf],
}

impl<'a> ShellCompleter<'a> {
    fn new(paths: &'a [PathBuf]) -> Self {
        Self { paths }
    }
}

impl<'a> rustyline::completion::Completer for ShellCompleter<'a> {
    type Candidate = ShellCompletionCandidate;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<ShellCompletionCandidate>)> {
        let trie = {
            let mut trie_builder = trie_builder_with_path_executables(self.paths);

            // Add built-in commands to trie builder.
            trie_builder.push("cd");
            trie_builder.push("echo");
            trie_builder.push("exit");
            trie_builder.push("pwd");
            trie_builder.push("type");

            trie_builder.build()
        };

        let completions = trie
            .postfix_search(line)
            .map(|completion: String| ShellCompletionCandidate::new(line, completion))
            .collect();

        Ok((pos, completions))
    }
}

struct ShellCompletionCandidate {
    display: String,
    replacement: String,
}

impl ShellCompletionCandidate {
    fn new(line: &str, completion: String) -> Self {
        let mut display = String::new();
        display.push_str(line);
        display.push_str(&completion);

        let mut replacement = completion;
        replacement.push(' ');

        Self {
            display,
            replacement,
        }
    }
}

impl Candidate for ShellCompletionCandidate {
    fn display(&self) -> &str {
        &self.display
    }

    fn replacement(&self) -> &str {
        &self.replacement
    }
}
