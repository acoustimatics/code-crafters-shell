mod ast;
mod parser;
mod scanner;

use crate::ast::*;
use crate::parser::*;
use std::io::{self, Write};

fn main() {
    repl();
}

/// Read, eval, print loop.
fn repl() -> ! {
    loop {
        match read_eval() {
            Ok(_) => {}
            Err(e) => eprintln!("{}", e),
        }
    }
}

/// Reads and evaluates a command.
fn read_eval() -> Result<(), String> {
    print!("$ ");
    let command_text = read()?;
    eval(&command_text)?;
    Ok(())
}

/// Reads a command.
fn read() -> Result<String, String> {
    match io::stdout().flush() {
        Ok(_) => {}
        Err(e) => return Err(format!("{}", e)),
    }

    let mut input = String::new();
    match io::stdin().read_line(&mut input) {
        Ok(_) => Ok(input),
        Err(e) => return Err(format!("{}", e)),
    }
}

/// Evaluates a command.
fn eval(command_text: &str) -> Result<(), String> {
    match parse(command_text)? {
        Command::Empty => Ok(()),
        Command::Echo(args) => {
            if args.len() > 0 {
                print!("{}", args[0]);
                for arg in args.iter().skip(1) {
                    print!(" {}", arg);
                }
            }
            println!("");
            Ok(())
        }
        Command::Exit(code) => {
            std::process::exit(code);
        }
        Command::External(args) => {
            assert!(args.len() > 0);
            let message = format!("{}: command not found", args[0]);
            Err(message)
        }
    }
}
