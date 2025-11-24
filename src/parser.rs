//! A command parser.

use anyhow::anyhow;

use crate::{
    ast::{BuiltIn, Command, Redirection },
    scanner::*,
};

/// Tracks and changes the state of the parser.
struct ParserState<'a> {
    /// A scanner to tokenize a command.
    scanner: Scanner<'a>,

    /// The current token in the command.
    current: Token,
}

impl<'a> ParserState<'a> {
    /// Creates and initialize a parser for a given command.
    fn new(command: &'a str) -> anyhow::Result<ParserState<'a>> {
        let mut scanner = Scanner::new(command);
        let current = scanner.next_token()?;
        Ok(ParserState { scanner, current })
    }

    /// Advances to the next token.
    fn advance(&mut self) -> anyhow::Result<()> {
        self.current = self.scanner.next_token()?;
        Ok(())
    }

    /// Advances to the next token, returning the current token before the
    /// advance.
    fn advance_keep_current(&mut self) -> anyhow::Result<Token> {
        let kept = std::mem::replace(&mut self.current, self.scanner.next_token()?);
        Ok(kept)
    }

    /// Advances to the next token if the given tag matches the current token's
    /// tag. Otherwise, an error is returned.
    fn expect(&mut self, expected_tag: TokenTag) -> anyhow::Result<()> {
        if self.current.tag == expected_tag {
            self.advance()?;
            Ok(())
        } else {
            Err(anyhow!(
                "expected `{:?}` but got `{}`",
                expected_tag,
                self.current.lexeme
            ))
        }
    }

    /// If the current token matches the given tag, advances to the next token
    /// and returns the matched lexeme. Otherwise, an error is returned.
    fn expect_lexeme(&mut self, tag: TokenTag) -> anyhow::Result<String> {
        if self.current.tag == tag {
            let token = self.advance_keep_current()?;
            Ok(token.lexeme)
        } else {
            Err(anyhow!(
                "expected `{:?}` but found `{}`",
                tag,
                self.current.lexeme
            ))
        }
    }
}

/// Parses a given command text.
pub fn parse(command_text: &str) -> anyhow::Result<Option<Command>> {
    let mut state = ParserState::new(command_text)?;
    match state.current.tag {
        TokenTag::Word => {
            let command = command(&mut state)?;
            Ok(Some(command))
        }
        TokenTag::EndOfCommand => Ok(None),
        tag => Err(anyhow!("unexpected token `{:?}`", tag)),
    }
}

fn command(state: &mut ParserState) -> anyhow::Result<Command> {
    assert!(state.current.tag == TokenTag::Word);

    let command = if let Some(built_in) = built_in(state)? {
        let redirection = redirection(state)?;
        Command::BuiltIn{ built_in, redirection }
    } else {
        let args = collect_integer_word(state)?;
        let redirection = redirection(state)?;
        Command::External { args, redirection }
    };

    state.expect(TokenTag::EndOfCommand)?;

    Ok(command)
}

fn redirection(state: &mut ParserState) -> anyhow::Result<Redirection> {
    use TokenTag::*;
    use Redirection::*;

    let redirection = match state.current.tag {
        RedirectOut | RedirectOutWithFileDescriptor(1) => StdOut { filename: redirection_filename(state)?, is_append: false },

        RedirectOutAppend | RedirectOutAppendWithFileDescriptor(1) => StdOut { filename: redirection_filename(state)?, is_append: true },

        RedirectOutWithFileDescriptor(2) => StdErr { filename: redirection_filename(state)?, is_append: false },
        RedirectOutAppendWithFileDescriptor(2) => StdErr { filename: redirection_filename(state)?, is_append: true },

        RedirectOutWithFileDescriptor(x) => Err(anyhow!("unrecognized file descriptor {x}"))?,
        RedirectOutAppendWithFileDescriptor(x) => {
            Err(anyhow!("unrecognized file descriptor {x}"))?
        }

        _ => None,
    };

    Ok(redirection)
}

fn redirection_filename(state: &mut ParserState) -> anyhow::Result<String> {
    // Advance past the redirection operator.
    state.advance()?;

    let filename = state.expect_lexeme(TokenTag::Word)?;
    Ok(filename)
}

fn built_in(state: &mut ParserState) -> anyhow::Result<Option<BuiltIn>> {
    assert!(state.current.tag == TokenTag::Word);
    let built_in = match state.current.lexeme.as_ref() {
        "cd" => cd(state)?,
        "echo" => echo(state)?,
        "exit" => exit(state)?,
        "pwd" => pwd(state)?,
        "type" => type_builtin(state)?,
        _ => return Ok(None),
    };
    Ok(Some(built_in))
}

/// Parses a cd command.
fn cd(state: &mut ParserState) -> anyhow::Result<BuiltIn> {
    assert!(state.current.tag == TokenTag::Word);
    assert!(state.current.lexeme == "cd");
    state.advance()?;
    let path = state.expect_lexeme(TokenTag::Word)?;
    Ok(BuiltIn::Cd(path))
}

/// Parses an echo commmand.
fn echo(state: &mut ParserState) -> anyhow::Result<BuiltIn> {
    assert!(state.current.tag == TokenTag::Word);
    assert!(state.current.lexeme == "echo");
    state.advance()?;
    let args = collect_integer_word(state)?;
    Ok(BuiltIn::Echo(args))
}

/// Parses an exit command.
fn exit(state: &mut ParserState) -> anyhow::Result<BuiltIn> {
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

/// Parses a pwd command.
fn pwd(state: &mut ParserState) -> anyhow::Result<BuiltIn> {
    assert!(state.current.tag == TokenTag::Word);
    assert!(state.current.lexeme == "pwd");
    state.advance()?;
    Ok(BuiltIn::Pwd)
}

/// Parses the `type` builtin.
fn type_builtin(state: &mut ParserState) -> anyhow::Result<BuiltIn> {
    assert!(state.current.tag == TokenTag::Word);
    assert!(state.current.lexeme == "type");
    state.advance()?;
    let command = state.expect_lexeme(TokenTag::Word)?;
    Ok(BuiltIn::Type(command))
}

/// Collects tokens into a vector as long as they are Word or Integer.
fn collect_integer_word(state: &mut ParserState) -> anyhow::Result<Vec<String>> {
    let mut items = Vec::new();
    while let TokenTag::Word | TokenTag::Integer(_) = state.current.tag {
        items.push(state.current.lexeme.clone());
        state.advance()?;
    }
    Ok(items)
}
