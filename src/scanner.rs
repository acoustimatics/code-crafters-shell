//! Scanner for the command line parser.

use std::str::Chars;

/// A token type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TokenTag {
    /// The end of the command text.
    EndOfCommand,

    /// An identifier.
    Identifier,

    /// An integer literal.
    Integer,
}

/// A token in a command text.
#[derive(Debug)]
pub struct Token {
    /// Tags what kind of token this is.
    pub tag: TokenTag,

    /// The token's text.
    pub lexeme: String,
}

impl Token {
    fn new(tag: TokenTag, lexeme: String) -> Token {
        Token { tag, lexeme }
    }
}

/// Converts a command's text into a stream of tokens.
pub struct Scanner<'a> {
    /// An iterator over the command text.
    chars: Chars<'a>,

    /// Current value from command text being considered.
    current: Option<char>,
}

impl<'a> Scanner<'a> {
    /// Creates a scanner for a give command text.
    pub fn new<'b>(command_text: &'b str) -> Scanner<'b> {
        let mut scanner = Scanner {
            chars: command_text.chars(),
            current: None,
        };
        scanner.advance();
        scanner
    }

    /// Returns the next token in the command text.
    pub fn next_token(&mut self) -> Result<Token, String> {
        self.skip_whitespace();

        let token = match self.current {
            None => Token::new(TokenTag::EndOfCommand, String::from("")),
            Some(c) if is_alpha(c) || c == '_' => {
                let lexeme = self.identifier();
                Token::new(TokenTag::Identifier, lexeme)
            }
            Some(c) if is_digit(c) => {
                let lexeme = self.integer();
                Token::new(TokenTag::Integer, lexeme)
            }
            Some(c) => {
                let msg = format!("unexpected character: {c}");
                return Err(msg);
            }
        };

        Ok(token)
    }

    /// Scans an identifier token.
    fn identifier(&mut self) -> String {
        let mut s = String::new();
        loop {
            match self.current {
                Some(c) if is_alpha(c) || is_digit(c) || c == '_' => {
                    s.push(c);
                    self.advance();
                }
                _ => break,
            }
        }
        s
    }

    /// Scans an integer token.
    fn integer(&mut self) -> String {
        let mut s = String::new();
        loop {
            match self.current {
                Some(c) if is_digit(c) => {
                    s.push(c);
                    self.advance();
                }
                _ => break,
            }
        }
        s
    }

    /// Advances the scanner past any whitespace.
    fn skip_whitespace(&mut self) {
        loop {
            match self.current {
                Some(c) if is_whitespace(c) => self.advance(),
                _ => break,
            }
        }
    }

    /// Advances `current` to the next character in command text.
    fn advance(&mut self) {
        self.current = self.chars.next();
    }
}

/// Determines if the given character is alphabetic.
fn is_alpha(c: char) -> bool {
    c.is_ascii_alphabetic()
}

/// Determines if the given character is a digit.
fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}

/// Determines if the given character is whitespace.
fn is_whitespace(c: char) -> bool {
    c == ' ' || c == '\t' || c == '\r' || c == '\n'
}
