//! Abstract syntax tree types for a command.

pub enum Pipeline {
    Single(Command),
    Double(Command, Command),
}

/// A shell command.
pub enum Command {
    BuiltIn {
        built_in: BuiltIn,
        redirection: Redirection,
    },
    External {
        args: Vec<String>,
        redirection: Redirection,
    },
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
