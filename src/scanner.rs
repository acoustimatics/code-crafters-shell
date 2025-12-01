//! Scanner for the command line parser.

use std::str::Chars;

use anyhow::anyhow;

/// A token type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TokenTag {
    /// The end of the command text.
    EndOfCommand,

    /// An integer literal.
    Integer(i32),
    
    /// A pipe operator `|`.
    Pipe,

    /// Output redirection operator `>`.
    RedirectOut,

    /// Output append redirection operator `>>`.
    RedirectOutAppend,

    /// Output redirection opterator with a file descriptor, e.g. `1>`.
    RedirectOutWithFileDescriptor(i32),

    /// Output redirection append opterator with a file descriptor, e.g. `1>>`.
    RedirectOutAppendWithFileDescriptor(i32),

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

    /// Previous character was a backspace.
    BackSpace,

    /// Previous character was a backspace inside double quotes.
    QuotedBackSpace,
}

/// Converts a command's text into a stream of tokens.
pub struct Scanner<'a> {
    /// An iterator over the command text.
    chars: Chars<'a>,

    /// Current value from command text being considered.
    current: Option<char>,

    /// Next char after current in the command text.
    next: Option<char>,
}

impl<'a> Scanner<'a> {
    /// Creates a scanner for a give command text.
    pub fn new<'b>(command_text: &'b str) -> Scanner<'b> {
        let mut scanner = Scanner {
            chars: command_text.chars(),
            current: None,
            next: None,
        };
        scanner.advance();
        scanner.advance();
        scanner
    }

    /// Returns the next token in the command text.
    pub fn next_token(&mut self) -> anyhow::Result<Token> {
        self.skip_whitespace();

        let token = match self.current {
            None => Token::new(TokenTag::EndOfCommand, String::from("")),
            Some('|') => {
                self.advance();
                let lexeme = String::from("|");
                Token::new(TokenTag::Pipe, lexeme)
            }
            Some('>') if matches!(self.next, Some('>')) => {
                self.advance();
                self.advance();
                let lexeme = String::from(">>");
                Token::new(TokenTag::RedirectOutAppend, lexeme)
            }
            Some('>') => {
                self.advance();
                let lexeme = String::from(">");
                Token::new(TokenTag::RedirectOut, lexeme)
            }
            Some(c) if is_digit(c) => self.integer()?,
            Some(_) => {
                let lexeme = self.word()?;
                Token::new(TokenTag::Word, lexeme)
            }
        };

        Ok(token)
    }

    /// Scans a quoted word.
    fn word(&mut self) -> anyhow::Result<String> {
        use WordState::*;

        let mut state = Normal;
        let mut s = String::new();

        loop {
            match (self.current, state) {
                (Some('\\'), Normal) => {
                    state = BackSpace;
                }

                (Some('\\'), InSingleQuote) => {
                    s.push('\\');
                }

                (Some('\\'), InDoubleQuote) => {
                    state = QuotedBackSpace;
                }

                (Some('\''), Normal) => {
                    state = InSingleQuote;
                }

                (Some('\''), InSingleQuote) => {
                    state = Normal;
                }

                (Some('\''), InDoubleQuote) => {
                    s.push('\'');
                }

                (Some('"'), Normal) => {
                    state = InDoubleQuote;
                }

                (Some('"'), InSingleQuote) => {
                    s.push('"');
                }

                (Some('"'), InDoubleQuote) => {
                    state = Normal;
                }

                (Some(c), Normal) if is_whitespace(c) => {
                    break;
                }

                (Some(c), InSingleQuote) if is_whitespace(c) => {
                    s.push(c);
                }

                (Some(c), InDoubleQuote) if is_whitespace(c) => {
                    s.push(c);
                }

                (Some(c), QuotedBackSpace) if c == '"' || c == '\\' => {
                    state = InDoubleQuote;
                    s.push(c);
                }

                (Some(c), QuotedBackSpace) => {
                    state = InDoubleQuote;
                    s.push('\\');
                    s.push(c);
                }

                (Some(c), BackSpace) => {
                    state = Normal;
                    s.push(c);
                }

                (Some(c), _) => {
                    s.push(c);
                }

                (None, Normal) => break,

                (None, InSingleQuote) => Err(anyhow!("unclosed single quote"))?,

                (None, InDoubleQuote) | (None, QuotedBackSpace) => {
                    Err(anyhow!("unclosed double quote"))?
                }

                (None, BackSpace) => Err(anyhow!("dangling back space"))?,
            }

            self.advance();
        }

        Ok(s)
    }

    /// Scans an integer token.
    fn integer(&mut self) -> anyhow::Result<Token> {
        let mut lexeme = String::new();
        loop {
            match self.current {
                Some(c) if is_digit(c) => {
                    lexeme.push(c);
                    self.advance();
                }
                _ => break,
            }
        }

        let i = parse_i32(&lexeme)?;

        let tag = match self.current {
            Some('>') if matches!(self.next, Some('>')) => {
                lexeme.push_str(">>");
                self.advance();
                self.advance();
                TokenTag::RedirectOutAppendWithFileDescriptor(i)
            }
            Some('>') => {
                lexeme.push('>');
                self.advance();
                TokenTag::RedirectOutWithFileDescriptor(i)
            }
            _ => TokenTag::Integer(i),
        };

        Ok(Token::new(tag, lexeme))
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
        self.current = self.next;
        self.next = self.chars.next();
    }
}

/// Parse string as an `i32` with a custom error result.
fn parse_i32(s: &str) -> anyhow::Result<i32> {
    match s.parse() {
        Ok(i) => Ok(i),
        Err(_) => Err(anyhow!("couldn't parse `{s}` as a signed 32 bit integer")),
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
