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

fn parse_args(args: &str) -> Vec<String> {
    // Split args by whitespace, but treat quoted strings as single arguments.
    // An unfinished quote is a single argument too.
    args.split('\'').enumerate().flat_map(|(i, part)| {
        // An example input/output is: hello "world program" test -> ["hello", "\"world program\"", "test"].
        // This makes sense from the code because the even indexed parts are split by whitespace, e.g. "hello " and " test",
        // while the odd indexed parts are turned back into quoted strings via format!("\"{}\"", part). The result is flattened into a single vector.
        if i % 2 == 0 {
            part.split_whitespace().map(|s| s.to_string()).collect::<Vec<String>>()
        } else {
            // Empty quotes '' are ignored.
            if part.is_empty() {
                vec![]
            } else {
                vec![part.to_string()]
            }
        }
    }).collect()
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
            "exit" => break,
            "echo" => {
                // Command	Expected output	Explanation
                // echo 'hello    world'	hello    world	Spaces are preserved within quotes.
                // echo hello    world	hello world	Consecutive spaces are collapsed unless quoted.
                // echo 'hello''world'	helloworld	Adjacent quoted strings 'hello' and 'world' are concatenated.
                // echo hello''world	helloworld	Empty quotes '' are ignored.
                let parsed_args = parse_args(args);
                println!("{}", parsed_args.join(" "));
            },
            "type" => if command_list.contains(&args) {
                println!("{} is a shell builtin", args);
            } else {
                if let Some(full_path) = find_executable_in_path(args) {
                    println!("{} is {}", args, full_path.display());
                } else {
                    println!("{}: not found", args);
                }
            },
            _ => if let Some(full_path) = find_executable_in_path(command) {
                let executable = full_path.file_name().unwrap(); // only the file name
                let args = parse_args(args);
                let status = std::process::Command::new(executable)
                    .args(args)
                    .status();
                match status {
                    Ok(status) => {
                        if !status.success() {
                            println!("{}: exited with status {}", command, status);
                        }
                    }
                    Err(e) => println!("{}: failed to execute: {}", command, e),
                }
            } else {
                println!("{}: command not found", command);
            }
        }
    }
}
