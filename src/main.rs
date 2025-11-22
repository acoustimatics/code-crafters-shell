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
use std::process::Stdio;

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
    let Some(command) = parse(command_text)? else {
        return Ok(());
    };
    eval_command(paths, command)
}

/// Evaluates a command.
fn eval_command(paths: &[PathBuf], command: Command) -> anyhow::Result<()> {
    match command.simple_command {
        SimpleCommand::BuiltIn(built_in) => {
            let (mut stdout, mut stderr) = match command.redirection {
                Some(redirection) => get_redirection_write(redirection)?,
                None => {
                    let stdout: Box<dyn Write> = Box::new(io::stdout());
                    let stderr: Box<dyn Write> = Box::new(io::stderr());
                    (stdout, stderr)
                }
            };
            eval_built_in(paths, &mut stdout, &mut stderr, built_in)
        }
        SimpleCommand::External(args) => {
            let (stdout, stderr) = match command.redirection {
                Some(redirection) => get_redirection_stdio(redirection)?,
                None => (Stdio::inherit(), Stdio::inherit()),
            };
            eval_external(args, stdout, stderr)
        }
    }
}

/// Evaluates a built in command.
fn eval_built_in(
    paths: &[PathBuf],
    stdout: &mut Box<dyn Write>,
    stderr: &mut Box<dyn Write>,
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
fn eval_external(args: Vec<String>, stdio: Stdio, stderr: Stdio) -> anyhow::Result<()> {
    assert!(!args.is_empty());
    let command = &args[0];
    let args = args.iter().skip(1);
    let status = std::process::Command::new(command)
        .args(args)
        .stdout(stdio)
        .stderr(stderr)
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

fn get_redirection_write(
    redirection: Redirection,
) -> anyhow::Result<(Box<dyn Write>, Box<dyn Write>)> {
    let file = create_redirection_file(redirection.target)?;

    match redirection.file_descriptor {
        FileDescriptor::StdOut => {
            let stdout: Box<dyn Write> = Box::new(file);
            let stderr: Box<dyn Write> = Box::new(io::stderr());
            Ok((stdout, stderr))
        }
        FileDescriptor::StdErr => {
            let stdout: Box<dyn Write> = Box::new(io::stdout());
            let stderr: Box<dyn Write> = Box::new(file);
            Ok((stdout, stderr))
        }
    }
}

fn get_redirection_stdio(redirection: Redirection) -> anyhow::Result<(Stdio, Stdio)> {
    let file = create_redirection_file(redirection.target)?;

    match redirection.file_descriptor {
        FileDescriptor::StdOut => {
            let stdio = Stdio::from(file);
            let stderr = Stdio::inherit();
            Ok((stdio, stderr))
        }
        FileDescriptor::StdErr => {
            let stdio = Stdio::inherit();
            let stderr = Stdio::from(file);
            Ok((stdio, stderr))
        }
    }
}

/// Creates a file for a redirection.
fn create_redirection_file(redirection_file: RedirectionFile) -> io::Result<File> {
    let mut open_options = OpenOptions::new();

    if redirection_file.is_append {
        open_options.append(true);
    } else {
        open_options.truncate(true);
    }

    open_options
        .write(true)
        .create(true)
        .open(redirection_file.name)
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
