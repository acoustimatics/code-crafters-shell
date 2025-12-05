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
use crate::parser::*;
use crate::system::*;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::{self, Cursor, Stdout, Write};
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
    let Some(pipeline) = parse(command_text)? else {
        return Ok(());
    };

    match pipeline {
        Pipeline::Single(Command::BuiltIn(built_in_command)) => {
            eval_built_in_command::<Stdout>(paths, built_in_command, None)?;
        }

        Pipeline::Single(Command::External(external_command)) => {
            let mut command = eval_external_command(external_command, None, None)?;
            let mut child = spawn_command(&mut command)?;
            let _status = child.wait()?;
        }

        Pipeline::Double(Command::BuiltIn(left_command), Command::BuiltIn(right_command)) => {
            eval_built_in_command::<Stdout>(paths, left_command, None)?;
            eval_built_in_command::<Stdout>(paths, right_command, None)?;
        }

        Pipeline::Double(Command::BuiltIn(left_command), Command::External(right_command)) => {
            let mut left_buffer = Cursor::new(Vec::new());
            eval_built_in_command(paths, left_command, Some(&mut left_buffer))?;

            let mut right_command =
                eval_external_command(right_command, Some(Stdio::piped()), None)?;
            let mut right_child = spawn_command(&mut right_command)?;
            if let Some(mut right_stdin) = right_child.stdin.take() {
                let left_buffer = left_buffer.into_inner();
                right_stdin.write_all(&left_buffer)?;
                right_stdin.flush()?;
            }
            let _right_status = right_child.wait()?;
        }

        Pipeline::Double(Command::External(left_command), Command::BuiltIn(right_command)) => {
            let mut left_command = eval_external_command(left_command, None, Some(Stdio::null()))?;
            let mut left_child = spawn_command(&mut left_command)?;
            let _left_status = left_child.wait()?;
            eval_built_in_command::<Stdout>(paths, right_command, None)?;
        }

        Pipeline::Double(Command::External(left_command), Command::External(right_command)) => {
            let left_stdout = Stdio::piped();
            let mut left_command = eval_external_command(left_command, None, Some(left_stdout))?;
            let mut left_child = spawn_command(&mut left_command)?;
            let right_stdin = left_child.stdout.take().map(Stdio::from);
            let mut right_command = eval_external_command(right_command, right_stdin, None)?;
            let mut right_child = spawn_command(&mut right_command)?;
            let _left_status = left_child.wait()?;
            let _right_status = right_child.wait()?;
        }
    }

    Ok(())
}

fn eval_built_in_command<TOut: Write>(
    paths: &[PathBuf],
    built_in_command: BuiltInCommand,
    stdout: Option<&mut TOut>,
) -> anyhow::Result<()> {
    match built_in_command.redirection {
        Redirection::StdOut {
            filename,
            is_append,
        } => {
            let mut stdout = open_file(filename, is_append)?;
            let mut stderr = io::stderr();
            eval_built_in(paths, &mut stdout, &mut stderr, built_in_command.built_in)?;
            Ok(())
        }

        Redirection::StdErr {
            filename,
            is_append,
        } => {
            let mut stdout = io::stdout();
            let mut stderr = open_file(filename, is_append)?;
            eval_built_in(paths, &mut stdout, &mut stderr, built_in_command.built_in)?;
            Ok(())
        }

        Redirection::None => {
            match stdout {
                None => {
                    let mut stdout = io::stdout();
                    let mut stderr = io::stderr();
                    eval_built_in(paths, &mut stdout, &mut stderr, built_in_command.built_in)?;
                }
                Some(stdout) => {
                    let mut stderr = io::stderr();
                    eval_built_in(paths, stdout, &mut stderr, built_in_command.built_in)?;
                }
            }
            Ok(())
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

fn eval_external_command(
    external_command: ExternalCommand,
    stdin: Option<Stdio>,
    stdout: Option<Stdio>,
) -> anyhow::Result<std::process::Command> {
    match external_command.redirection {
        Redirection::StdOut {
            filename,
            is_append,
        } => {
            let stdin = stdin.unwrap_or_else(Stdio::inherit);
            let stdout = Stdio::from(open_file(filename, is_append)?);
            let stderr = Stdio::inherit();
            let command = eval_external(external_command.args, stdin, stdout, stderr)?;
            Ok(command)
        }

        Redirection::StdErr {
            filename,
            is_append,
        } => {
            let stdin = stdin.unwrap_or_else(Stdio::inherit);
            let stdout = stdout.unwrap_or_else(Stdio::inherit);
            let stderr = Stdio::from(open_file(filename, is_append)?);
            let command = eval_external(external_command.args, stdin, stdout, stderr)?;
            Ok(command)
        }

        Redirection::None => {
            let stdin = stdin.unwrap_or_else(Stdio::inherit);
            let stdout = stdout.unwrap_or_else(Stdio::inherit);
            let stderr = Stdio::inherit();
            let command = eval_external(external_command.args, stdin, stdout, stderr)?;
            Ok(command)
        }
    }
}

/// Evaluates an external command, e.g. `cd`.
fn eval_external(
    args: Vec<String>,
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
