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
            _ => if let Some(full_path) = find_executable_in_path(command) {
                let executable = full_path.file_name().unwrap(); // only the file name
                // Split args by whitespace, but treat quoted strings as single arguments.
                // An unfinished quote is a single argument too.
                let args = args.split('"').enumerate().flat_map(|(i, part)| {
                    if i % 2 == 0 {
                        part.split_whitespace().map(|s| s.to_string()).collect::<Vec<String>>()
                    } else {
                        /* I thought that I had to provide the quotes with the quoted string to Python.
                        std::process::Command expects raw arguments (without shell quotes).

                        When you run python3 -c "import sys; ...":
                        Standard Shell: The shell parses the quotes to treat everything inside as one argument, but strips the quotes before passing the string import sys; ... to Python.
                        Your Shell (Previously):
                        - You are parsing the quotes to group the argument, but then adding them back with format!("\"{}\"", part).
                        - Python receives: "import sys; print(sys.executable)" (as a string literal including quotes).
                        - Python executes this "code". Since it's just a string literal statement, it evaluates it and does nothing (no output).
                         */
                        vec![part.to_string()]
                    }
                });
                // An example input/output is: hello "world program" test -> ["hello", "\"world program\"", "test"].
                // This makes sense from the above code because the even indexed parts are split by whitespace, e.g. "hello " and " test",
                // while the odd indexed parts are turned back into quoted strings via format!("\"{}\"", part). The result is flattened into a single vector.
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
