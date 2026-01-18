#[allow(unused_imports)]
use std::io::{self, Write};

fn main() {
    while true {
        print!("$ ");
        io::stdout().flush().unwrap();
        let mut console_input = String::new();
        let _ = io::stdin().read_line(&mut console_input);
        console_input = console_input.trim().to_string();
        let (command, args) = console_input.split_once(' ').unwrap_or((&console_input, ""));
        match command {
            "exit" => break,
            "echo" => println!("{}", args),
            _ => println!("{}: command not found", command),
        }
    }
}
