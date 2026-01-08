#[allow(unused_imports)]
use std::io::{self, Write};

fn main() {
    let mut console_input = String::new();
    let command_list: Vec<&str> = vec![];
    print!("$ ");
    io::stdout().flush().unwrap();
    let _ = io::stdin().read_line(&mut console_input);
    let command = console_input.trim();
    if command_list.contains(&command) {
        println!("Command found: {}", command);
    } else {
        println!("{}: command not found", command);
    }
}
