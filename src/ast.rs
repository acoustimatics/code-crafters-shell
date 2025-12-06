//! Abstract syntax tree types for a command.

/// A shell command.
pub enum Command {
    BuiltIn(BuiltInCommand),
    External(ExternalCommand),
}

/// Contents of a built-in command.
pub struct BuiltInCommand {
    pub built_in: BuiltIn,
    pub redirection: Redirection,
}

/// Contents of an external command.
pub struct ExternalCommand {
    pub args: Vec<String>,
    pub redirection: Redirection,
}

/// A shell command.
#[derive(Debug)]
pub enum BuiltIn {
    /// Changes the working directory to a given path.
    Cd(String),

    /// Echos back user input.
    Echo(Vec<String>),

    /// Exits the shell with a return code.
    Exit(i32),

    /// Prints the working directory.
    Pwd,

    /// Displays the type of command.
    Type(String),
}

pub enum Redirection {
    None,
    StdOut { filename: String, is_append: bool },
    StdErr { filename: String, is_append: bool },
}
