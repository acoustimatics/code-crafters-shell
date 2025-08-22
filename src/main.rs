#[allow(unused_imports)]
use std::io::{self, Write};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Uncomment this block to pass the first stage
    print!("$ ");
    io::stdout().flush()?;

    // Wait for user input
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    println!("{}: command not found", input.trim());

    Ok(())
}
