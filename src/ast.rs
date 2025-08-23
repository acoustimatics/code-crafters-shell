//! Abstract syntax tree types for a command.

/// A shell command.
#[derive(Debug)]
pub enum Command {
    /// An empty command.
    Empty,

    /// Exits the shell with a return code.
    Exit(i32),

    /// An external command.
    External(Vec<String>),
}
