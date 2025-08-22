use std::io::{self, Write};
use std::error::Error;

fn main() {
    repl();
}

fn repl() -> ! {
    loop {
        print!("$ ");
        match read() {
            Ok(input) => println!("{}: command not found", input.trim()),
            Err(e) => eprintln!("error: {}", e),
        }
    }
}

fn read() -> Result<String, Box<dyn Error>> {
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input)
}
