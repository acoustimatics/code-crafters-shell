//! A command parser.

use anyhow::anyhow;

use crate::ast::*;
use crate::scanner::{Scanner, TokenTag};
use parser_state::ParserState;

type PS<'a> = ParserState<Scanner<'a>>;

/// Parses a given command text. Returns an array of commands which represents
/// a pipeline.
pub fn parse(command_text: &str) -> anyhow::Result<Vec<Command>> {
    let scanner = Scanner::new(command_text);
    let mut state = ParserState::new(scanner)?;
    match state.current.tag {
        TokenTag::Word => {
            let pipeline = pipeline(&mut state)?;
            Ok(pipeline)
        }
        TokenTag::EndOfCommand => Ok(Vec::new()),
        tag => Err(anyhow!("unexpected token `{:?}`", tag)),
    }
}

/// Parses a pipeline of commands. Returns a vector of all commands in the
/// pipeline in order.
fn pipeline(state: &mut PS) -> anyhow::Result<Vec<Command>> {
    let mut commands = Vec::new();

    let mut parse_another_command = true;
    while parse_another_command {
        let command = command(state)?;
        commands.push(command);
        parse_another_command = state.matches(TokenTag::Pipe)?;
    }

    Ok(commands)
}

fn command(state: &mut PS) -> anyhow::Result<Command> {
    assert!(state.current.tag == TokenTag::Word);

    let command = if let Some(built_in) = built_in(state)? {
        let redirection = redirection(state)?;
        let built_in_command = BuiltInCommand {
            built_in,
            redirection,
        };
        Command::BuiltIn(built_in_command)
    } else {
        let args = collect_integer_word(state)?;
        let redirection = redirection(state)?;
        let external_command = ExternalCommand { args, redirection };
        Command::External(external_command)
    };

    Ok(command)
}

fn redirection(state: &mut PS) -> anyhow::Result<Redirection> {
    use Redirection::*;
    use TokenTag::*;

    let redirection = match state.current.tag {
        RedirectOut | RedirectOutWithFileDescriptor(1) => StdOut {
            filename: redirection_filename(state)?,
            is_append: false,
        },

        RedirectOutAppend | RedirectOutAppendWithFileDescriptor(1) => StdOut {
            filename: redirection_filename(state)?,
            is_append: true,
        },

        RedirectOutWithFileDescriptor(2) => StdErr {
            filename: redirection_filename(state)?,
            is_append: false,
        },
        RedirectOutAppendWithFileDescriptor(2) => StdErr {
            filename: redirection_filename(state)?,
            is_append: true,
        },

        RedirectOutWithFileDescriptor(x) => Err(anyhow!("unrecognized file descriptor {x}"))?,
        RedirectOutAppendWithFileDescriptor(x) => Err(anyhow!("unrecognized file descriptor {x}"))?,

        _ => None,
    };

    Ok(redirection)
}

fn redirection_filename(state: &mut PS) -> anyhow::Result<String> {
    // Advance past the redirection operator.
    state.advance()?;

    let filename = state.expect_lexeme(TokenTag::Word)?;
    Ok(filename)
}

fn built_in(state: &mut PS) -> anyhow::Result<Option<BuiltIn>> {
    assert!(state.current.tag == TokenTag::Word);
    let built_in = match state.current.lexeme.as_ref() {
        "cd" => cd(state)?,
        "echo" => echo(state)?,
        "exit" => exit(state)?,
        "history" => history(state)?,
        "pwd" => pwd(state)?,
        "type" => type_builtin(state)?,
        _ => return Ok(None),
    };
    Ok(Some(built_in))
}

/// Parses a cd command.
fn cd(state: &mut PS) -> anyhow::Result<BuiltIn> {
    assert!(state.current.tag == TokenTag::Word);
    assert!(state.current.lexeme == "cd");
    state.advance()?;
    let path = state.expect_lexeme(TokenTag::Word)?;
    Ok(BuiltIn::Cd(path))
}

/// Parses an echo commmand.
fn echo(state: &mut PS) -> anyhow::Result<BuiltIn> {
    assert!(state.current.tag == TokenTag::Word);
    assert!(state.current.lexeme == "echo");
    state.advance()?;
    let args = collect_integer_word(state)?;
    Ok(BuiltIn::Echo(args))
}

/// Parses an exit command.
fn exit(state: &mut PS) -> anyhow::Result<BuiltIn> {
    assert!(state.current.tag == TokenTag::Word);
    assert!(state.current.lexeme == "exit");

    state.advance()?;

    let status = match state.current.tag {
        TokenTag::Integer(status) => {
            state.advance()?;
            status
        }
        _ => 0,
    };

    Ok(BuiltIn::Exit(status))
}

fn history(state: &mut PS) -> anyhow::Result<BuiltIn> {
    state.advance()?;

    Ok(BuiltIn::History)
}

/// Parses a pwd command.
fn pwd(state: &mut PS) -> anyhow::Result<BuiltIn> {
    assert!(state.current.tag == TokenTag::Word);
    assert!(state.current.lexeme == "pwd");
    state.advance()?;
    Ok(BuiltIn::Pwd)
}

/// Parses the `type` builtin.
fn type_builtin(state: &mut PS) -> anyhow::Result<BuiltIn> {
    assert!(state.current.tag == TokenTag::Word);
    assert!(state.current.lexeme == "type");
    state.advance()?;
    let command = state.expect_lexeme(TokenTag::Word)?;
    Ok(BuiltIn::Type(command))
}

/// Collects tokens into a vector as long as they are Word or Integer.
fn collect_integer_word(state: &mut PS) -> anyhow::Result<Vec<String>> {
    let mut items = Vec::new();
    while let TokenTag::Word | TokenTag::Integer(_) = state.current.tag {
        items.push(state.current.lexeme.clone());
        state.advance()?;
    }
    Ok(items)
}
