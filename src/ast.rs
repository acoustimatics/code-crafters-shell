//! Abstract syntax tree types for a command.

/// A shell command.
pub struct Command {
    /// A simple command.
    pub simple_command: SimpleCommand,

    /// IO redirection for the simple command.
    pub redirection: Option<Redirection>,
}

impl Command {
    pub fn new(simple_command: SimpleCommand, redirection: Option<Redirection>) -> Command {
        Command {
            simple_command,
            redirection,
        }
    }
}

/// A simple command without redirects.
pub enum SimpleCommand {
    /// A built in command.
    BuiltIn(BuiltIn),

    /// An external command, e.g. `ls -a`.
    External(Vec<String>),
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

/// Output redirection operator.
pub struct Redirection {
    /// The source file of the redirection.
    pub file_descriptor: FileDescriptor,

    /// The target file of the redirection.
    pub target: RedirectionFile,
}

/// Enumerates redirection file descriptors.
pub enum FileDescriptor {
    StdOut,
    StdErr,
}

/// Represents how to create a file for output redirection.
pub struct RedirectionFile {
    /// A file name.
    pub name: String,

    /// Whether redirection appends or completely overwrites.
    pub is_append: bool,
}
