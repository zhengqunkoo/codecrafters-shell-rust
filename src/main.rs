#[allow(unused_imports)]
use std::io::{self, Write};

fn main() {
    let command_list: Vec<&str> = vec!["exit", "echo", "type"];
    while true {
        print!("$ ");
        io::stdout().flush().unwrap();
        let mut console_input = String::new();
        let _ = io::stdin().read_line(&mut console_input);
        console_input = console_input.trim().to_string();
        let (command, args) = console_input.split_once(' ').unwrap_or((&console_input, ""));
        match command {
            cmd if command_list.contains(&cmd) => {
                match cmd {
                    "exit" => break,
                    "echo" => println!("{}", args),
                    "type" => if command_list.contains(&args) {
                        println!("{} is a shell builtin", args);
                    } else {
                        println!("{}: not found", args);
                    },
                    _ => {}
                }
            }
            _ => println!("{}: command not found", command),
        }
    }
}
