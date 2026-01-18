#[allow(unused_imports)]
use std::env;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;

fn find_executable_in_path(executable: &str) -> Option<std::path::PathBuf> {
    let path_env = env::var("PATH").unwrap_or_else(|_| String::new());
    for path_dir in path_env.split(':') {
        let full_path = std::path::Path::new(path_dir).join(executable);
        if let Ok(metadata) = std::fs::metadata(&full_path) {
            if metadata.permissions().mode() & 0o111 != 0 { // if any execute bit is set
                return Some(full_path);
            }
        }
    }
    None
}

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
                        if let Some(full_path) = find_executable_in_path(args) {
                            println!("{} is {}", args, full_path.display());
                        } else {
                            println!("{}: not found", args);
                        }
                    },
                    _ => {}
                }
            }
            _ => println!("{}: command not found", command),
        }
    }
}
