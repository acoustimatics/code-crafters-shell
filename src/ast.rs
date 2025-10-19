//! Abstract syntax tree types for a command.

/// A shell command.
#[derive(Debug)]
pub enum Command {
    /// Changes the working directory to a given path.
    Cd(String),

    /// An empty command.
    Empty,

    /// Echos back user input.
    Echo(Vec<String>),

    /// Exits the shell with a return code.
    Exit(i32),

    /// An external command.
    External(Vec<String>),

    /// Prints the working directory.
    Pwd,

    /// Displays the type of command.
    Type(String),
}
