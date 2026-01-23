#[allow(unused_imports)]
use std::env;

#[cfg(test)]
mod tests;

use std::io::{self, Write};
#[cfg(target_family = "unix")]
use std::os::unix::fs::PermissionsExt;

pub fn find_executable_in_path(executable: &str, path_env_opt: Option<&str>) -> Option<std::path::PathBuf> {
    let default_path;
    let path_to_use = match path_env_opt {
        Some(p) => p,
        None => {
            default_path = env::var("PATH").unwrap_or_default();
            &default_path
        }
    };

    let splitter = if cfg!(windows) { ';' } else { ':' };
    for path_dir in path_to_use.split(splitter) {
        let full_path = std::path::Path::new(path_dir).join(executable);
        if let Ok(_metadata) = std::fs::metadata(&full_path) {
            #[cfg(target_family = "unix")]
            if _metadata.permissions().mode() & 0o111 != 0 { // if any execute bit is set
                return Some(full_path);
            }
            #[cfg(target_family = "windows")]
            // On Windows, existence is a basic check. Real shells check PATHEXT, etc.
            return Some(full_path);
        }
    }
    None
}

pub fn parse_command(input: &str) -> (String, Vec<String>, Option<String>) {
    let input = input.trim();
    let (command, rest) = input.split_once(' ').unwrap_or((input, ""));

    let (args, filename) = if let Some((a, f)) = rest.split_once("1>") {
        (parse_args(a), Some(f.trim().trim_matches('"').trim_matches('\'').to_string()))
    } else if let Some((a, f)) = rest.split_once('>') {
        (parse_args(a), Some(f.trim().trim_matches('"').trim_matches('\'').to_string()))
    } else {
        (parse_args(rest), None)
    };

    (command.to_string(), args, filename)
}

pub fn parse_args(args: &str) -> Vec<String> {
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
    let command_list: Vec<String> = vec!["exit", "echo", "type", "pwd", "cd"].into_iter().map(String::from).collect();
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();
        let mut console_input = String::new();
        let _ = io::stdin().read_line(&mut console_input);
        let (command, args, filename_opt) = parse_command(&console_input);
        let filename = filename_opt.as_deref().unwrap_or("");

        // `string_for_stdout`` will either be printed to the console, or written to `filename`.
        // Errors are printed to the console directly.
        let mut string_for_stdout = String::new();
        match command.as_str() {
            "exit" => break,
            "echo" => {
                // Command	Expected output	Explanation
                // echo 'hello    world'	hello    world	Spaces are preserved within quotes.
                // echo hello    world	hello world	Consecutive spaces are collapsed unless quoted.
                // echo 'hello''world'	helloworld	Adjacent quoted strings 'hello' and 'world' are concatenated.
                // echo hello''world	helloworld	Empty quotes '' are ignored.
                string_for_stdout = args.join(" ") + "\n";
            },
            "type" => for arg in args {
                if command_list.contains(&arg) {
                    string_for_stdout.push_str(&format!("{} is a shell builtin\n", arg));
                } else if let Some(full_path) = find_executable_in_path(&arg, None) {
                    string_for_stdout.push_str(&format!("{} is {}\n", arg, full_path.display()));
                } else {
                    string_for_stdout.push_str(&format!("{}: not found\n", arg));
                }
            },
            "pwd" => {
                match env::current_dir() {
                    Ok(path) => string_for_stdout = path.display().to_string() + "\n",
                    Err(e) => println!("pwd: error retrieving current directory: {}", e),
                }
            },
            "cd" => {
                if args.len() > 1 {
                    println!("cd: too many arguments");
                    continue;
                }
                let target_dir = if args.len() == 0 || args[0] == "~" {
                        env::var("HOME").unwrap_or_else(|_| String::new())
                    } else {
                        args[0].to_string()
                    };
                if let Err(_) = env::set_current_dir(&target_dir) { // this also handles relative paths
                    println!("cd: {}: No such file or directory", target_dir);
                }
            },
            "" => continue, // empty input, just reprompt
            _ => if let Some(full_path) = find_executable_in_path(&command, None) {
                let executable = full_path.file_name().unwrap(); // only the file name
                let mut cmd = std::process::Command::new(executable);
                cmd.args(args);

                if !filename.is_empty() {
                    if let Ok(file) = std::fs::File::create(filename) {
                        cmd.stdout(file);
                    }
                } // else cmd prints to stdout

                let status = cmd.status();
                match status {
                    Ok(status) => {
                        if !status.success() {
                            //println!("{}: exited with status {}", command, status);
                        }
                    }
                    Err(e) => println!("{}: failed to execute: {}", command, e),
                }
                continue; // This is necessary to prevent my shell from overwriting the external command's work with an empty string in the postprocessing step below.
            } else {
                println!("{}: command not found", command);
            }
        }

        // Handle filename. Open file descriptor
        if filename.is_empty() {
            // No output redirection, print to console
            print!("{}", string_for_stdout);
        } else {
            // Redirect output to file
            let stdout_file = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(filename);
            
            match stdout_file {
                Ok(mut file) => {
                    write!(file, "{}", string_for_stdout).unwrap();
                },
                Err(_) => {
                    println!("{}: cannot open file for output redirection", filename);
                }
            }
        }
    }
}
