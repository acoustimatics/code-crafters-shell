//! A command parser.

use crate::{ast::Command, scanner::*};

/// Tracks and changes the state of the parser.
struct ParserState<'a> {
    /// A scanner to tokenize a command.
    scanner: Scanner<'a>,

    /// The current token in the command.
    current: Token,
}

impl<'a> ParserState<'a> {
    /// Creates and initialize a parser for a given command.
    fn new(command: &'a str) -> Result<ParserState<'a>, String> {
        let mut scanner = Scanner::new(command);
        let current = scanner.next_token()?;
        Ok(ParserState { scanner, current })
    }

    /// Advances to the next token.
    fn advance(&mut self) -> Result<(), String> {
        self.current = self.scanner.next_token()?;
        Ok(())
    }

    /// Advances to the next token if the given tag matches the current token's
    /// tag. Otherwise, an error is returned.
    fn expect(&mut self, expected_tag: TokenTag) -> Result<(), String> {
        if self.current.tag == expected_tag {
            self.advance()?;
            Ok(())
        } else {
            let message = format!(
                "expected `{:?}` but got `{}`",
                expected_tag, self.current.lexeme
            );
            Err(message)
        }
    }

    /// If the current token matches the given tag, advances to the next token
    /// and returns the matched lexeme. Otherwise, an error is returned.
    fn expect_lexeme(&mut self, tag: TokenTag) -> Result<String, String> {
        if self.current.tag == tag {
            let lexeme = self.current.lexeme.clone();
            self.advance()?;
            Ok(lexeme)
        } else {
            let message = format!("expected `{:?}` but found `{}`", tag, self.current.lexeme);
            Err(message)
        }
    }

    /// Returns whether the tag matches the current token. If the tags match
    /// then we advance to the next token.
    fn is_match(&mut self, tag: TokenTag) -> Result<bool, String> {
        let is_match = self.current.tag == tag;
        if is_match {
            self.advance()?;
        }
        Ok(is_match)
    }
}

/// Parses a given command text.
pub fn parse(command_text: &str) -> Result<Command, String> {
    let mut state = ParserState::new(command_text)?;
    command(&mut state)
}

/// Parses a command.
fn command(state: &mut ParserState) -> Result<Command, String> {
    if state.is_match(TokenTag::EndOfCommand)? {
        Ok(Command::Empty)
    } else if state.current.tag == TokenTag::Word {
        let command = match state.current.lexeme.as_ref() {
            "cd" => cd(state)?,
            "echo" => echo(state)?,
            "exit" => exit(state)?,
            "pwd" => pwd(state)?,
            "type" => type_builtin(state)?,
            _ => external(state)?,
        };
        state.expect(TokenTag::EndOfCommand)?;
        Ok(command)
    } else {
        unimplemented!()
    }
}

/// Parses a cd command.
fn cd(state: &mut ParserState) -> Result<Command, String> {
    assert!(state.current.tag == TokenTag::Word);
    assert!(state.current.lexeme == "cd");
    state.advance()?;
    let path = state.expect_lexeme(TokenTag::Word)?;
    Ok(Command::Cd(path))
}

/// Parses an echo commmand.
fn echo(state: &mut ParserState) -> Result<Command, String> {
    assert!(state.current.tag == TokenTag::Word);
    assert!(state.current.lexeme == "echo");
    state.advance()?;
    let mut args = Vec::new();
    while state.current.tag != TokenTag::EndOfCommand {
        args.push(state.current.lexeme.clone());
        state.advance()?;
    }
    Ok(Command::Echo(args))
}

/// Parses an exit command.
fn exit(state: &mut ParserState) -> Result<Command, String> {
    assert!(state.current.tag == TokenTag::Word);
    assert!(state.current.lexeme == "exit");
    state.advance()?;
    let integer_string = state.expect_lexeme(TokenTag::Integer)?;
    match integer_string.parse() {
        Ok(integer) => Ok(Command::Exit(integer)),
        Err(_) => {
            let message = format!(
                "couldn't parse `{}` as a signed 32 bit integer",
                integer_string
            );
            Err(message)
        }
    }
}

/// Parses a pwd command.
fn pwd(state: &mut ParserState) -> Result<Command, String> {
    assert!(state.current.tag == TokenTag::Word);
    assert!(state.current.lexeme == "pwd");
    state.advance()?;
    Ok(Command::Pwd)
}

/// Parses the `type` builtin.
fn type_builtin(state: &mut ParserState) -> Result<Command, String> {
    assert!(state.current.tag == TokenTag::Word);
    assert!(state.current.lexeme == "type");
    state.advance()?;
    let command = state.expect_lexeme(TokenTag::Word)?;
    Ok(Command::Type(command))
}

/// Parses an external command.
fn external(state: &mut ParserState) -> Result<Command, String> {
    assert!(state.current.tag == TokenTag::Word);
    let mut args = Vec::new();
    while state.current.tag != TokenTag::EndOfCommand {
        args.push(state.current.lexeme.clone());
        state.advance()?;
    }
    Ok(Command::External(args))
}
