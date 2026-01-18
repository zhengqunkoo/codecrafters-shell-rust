#[allow(unused_imports)]
use std::io::{self, Write};

fn main() {
    let command_list: Vec<&str> = vec!["exit"];
    while true {
        print!("$ ");
        io::stdout().flush().unwrap();
        let mut console_input = String::new();
        let _ = io::stdin().read_line(&mut console_input);
        let command = console_input.trim();
        if command_list.contains(&command) {
            if command == "exit" {
                break;
            }
        } else {
            println!("{}: command not found", command);
        }
    }
}
