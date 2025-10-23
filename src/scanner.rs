//! Scanner for the command line parser.

use std::str::Chars;

/// A token type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TokenTag {
    /// The end of the command text.
    EndOfCommand,

    /// An integer literal.
    Integer,

    /// A word which is a string of non-whitespace characters that doesn't
    /// start with a digit.
    Word,
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

/// Possible states when scanning a word token.
#[derive(Clone, Copy)]
enum WordState {
    /// Normal state.
    Normal,
    
    /// Inside single quoted text.
    InSingleQuote, 
    
    /// Inside double quoted text.
    InDoubleQuote,
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
            Some(c) if is_digit(c) => {
                let lexeme = self.integer();
                Token::new(TokenTag::Integer, lexeme)
            }
            Some(_) => {
                let lexeme = self.word()?;
                Token::new(TokenTag::Word, lexeme)
            }
        };

        Ok(token)
    }

    /// Scans a quoted word.
    fn word(&mut self) -> Result<String, String> {
        use WordState::*;

        let mut state = Normal;
        let mut s = String::new();

        loop {
            match (self.current, state) {
                (Some('\''), Normal) => {
                    state = InSingleQuote;
                    self.advance();
                }

                (Some('\''), InSingleQuote) => {
                    state = Normal;
                    self.advance();
                }

                (Some('\''), InDoubleQuote) => {
                    s.push('\'');
                    self.advance();
                }

                (Some('"'), Normal) => {
                    state = InDoubleQuote;
                    self.advance();
                }

                (Some('"'), InSingleQuote) => {
                    s.push('"');
                    self.advance();
                }

                (Some('"'), InDoubleQuote) => {
                    state = Normal;
                    self.advance();
                }

                (Some(c), Normal) if is_whitespace(c) => {
                    break;
                }

                (Some(c), InSingleQuote) if is_whitespace(c) => {
                    s.push(c);
                    self.advance();
                }

                (Some(c), InDoubleQuote) if is_whitespace(c) => {
                    s.push(c);
                    self.advance();
                }

                (Some(c), _) => {
                    s.push(c);
                    self.advance();
                }

                (None, Normal) => break,

                (None, InSingleQuote) => {
                    let message = String::from("unclosed single quote");
                    return Err(message);
                }

                (None, InDoubleQuote) => {
                    let message = String::from("unclosed double quote");
                    return Err(message);
                }
            }
        }

        Ok(s)
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

/// Determines if the given character is a digit.
fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}

/// Determines if the given character is whitespace.
fn is_whitespace(c: char) -> bool {
    c == ' ' || c == '\t' || c == '\r' || c == '\n'
}
