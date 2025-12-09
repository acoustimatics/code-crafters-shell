mod ast;
mod error;
mod parser;
mod scanner;
mod system;

use rustyline::completion::Candidate;
use rustyline::{
    Completer, CompletionType, Config, Context, Editor, Helper, Highlighter, Hinter, Validator,
};

use crate::ast::*;
use crate::parser::*;
use crate::system::*;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::{self, Cursor, Write};
use std::path::PathBuf;
use std::process::{Child, Stdio};

fn main() -> anyhow::Result<()> {
    let paths = get_path();
    let mut history = Vec::new();

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
        history.push(command_text);
        if let Err(e) = eval(&paths, &history) {
            eprintln!("{}", e);
        }
    }
}

fn eval(paths: &[PathBuf], history: &[String]) -> anyhow::Result<()> {
    assert!(!history.is_empty());

    let command_text = &history[history.len() - 1];
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
fn eval_built_in_command(
    paths: &[PathBuf],
    history: &[String],
    built_in_command: &BuiltInCommand,
) -> anyhow::Result<Vec<u8>> {
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
fn eval_built_in<TOut: Write, TErr: Write>(
    paths: &[PathBuf],
    history: &[String],
    stdout: &mut TOut,
    stderr: &mut TErr,
    built_in: &BuiltIn,
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
            let skip = match limit {
                Some(limit) if *limit >= history.len() => 0,
                Some(limit) => history.len() - limit,
                None => 0,
            };
            for (i, command_text) in history.iter().enumerate().skip(skip) {
                writeln!(stdout, "\t{}\t{}", i + 1, command_text)?;
            }
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
