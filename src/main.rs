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

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum RedirectTo {
    Stdout,
    Stderr,
    StdoutAppend,
    StderrAppend,
}

pub fn parse_command(input: &str) -> (String, Vec<String>, Option<String>, Option<RedirectTo>) {
    let input = input.trim();
    let (command, rest) = input.split_once(' ').unwrap_or((input, ""));

    let (args, filename, redirect_to) = if let Some((a, f)) = rest.split_once("1>>") {
        (parse_args(a), Some(f.trim().trim_matches('"').trim_matches('\'').to_string()), Some(RedirectTo::StdoutAppend))
    } else if let Some((a, f)) = rest.split_once("2>>") {
        (parse_args(a), Some(f.trim().trim_matches('"').trim_matches('\'').to_string()), Some(RedirectTo::StderrAppend))
    } else if let Some((a, f)) = rest.split_once(">>") {
        (parse_args(a), Some(f.trim().trim_matches('"').trim_matches('\'').to_string()), Some(RedirectTo::StdoutAppend))
    } else if let Some((a, f)) = rest.split_once("1>") {
        (parse_args(a), Some(f.trim().trim_matches('"').trim_matches('\'').to_string()), Some(RedirectTo::Stdout))
    } else if let Some((a, f)) = rest.split_once("2>") {
        (parse_args(a), Some(f.trim().trim_matches('"').trim_matches('\'').to_string()), Some(RedirectTo::Stderr))
    } else if let Some((a, f)) = rest.split_once('>') {
        (parse_args(a), Some(f.trim().trim_matches('"').trim_matches('\'').to_string()), Some(RedirectTo::Stdout))
    } else {
        (parse_args(rest), None, None)
    };

    (command.to_string(), args, filename, redirect_to)
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

pub fn execute_command(command: &str, args: Vec<String>, filename: &str, redirect_to: Option<RedirectTo>) -> bool {
    let command_list: Vec<String> = vec!["exit", "echo", "type", "pwd", "cd"].into_iter().map(String::from).collect();
    let mut string_for_stdout = String::new();
    let mut string_for_stderr = String::new();

    match command {
        "exit" => return false,
        "echo" => {
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
                Err(e) => string_for_stderr = format!("pwd: error retrieving current directory: {}\n", e),
            }
        },
        "cd" => {
            if args.len() > 1 {
                string_for_stderr = "cd: too many arguments\n".to_string();
            } else {
                let target_dir = if args.len() == 0 || args[0] == "~" {
                        env::var("HOME").unwrap_or_else(|_| String::new())
                    } else {
                        args[0].to_string()
                    };
                if let Err(_) = env::set_current_dir(&target_dir) {
                    string_for_stderr = format!("cd: {}: No such file or directory\n", target_dir);
                }
            }
        },
        "" => return true,
        _ => if let Some(full_path) = find_executable_in_path(&command, None) {
            let executable = full_path.file_name().unwrap();
            let mut cmd = std::process::Command::new(executable);
            cmd.args(args);

            if !filename.is_empty() {
                let mut fs_open_options = std::fs::OpenOptions::new();
                fs_open_options.create(true).write(true);
                match redirect_to {
                    Some(RedirectTo::Stdout) => { fs_open_options.truncate(true); }
                    Some(RedirectTo::Stderr) => { fs_open_options.truncate(true); }
                    Some(RedirectTo::StdoutAppend) => { fs_open_options.append(true); }
                    Some(RedirectTo::StderrAppend) => { fs_open_options.append(true); }
                    None => {}
                }

                match fs_open_options.open(filename) {
                    Ok(file) => {
                        match redirect_to {
                            Some(RedirectTo::Stdout) | Some(RedirectTo::StdoutAppend) => {
                                cmd.stdout(file);
                            }
                            Some(RedirectTo::Stderr) | Some(RedirectTo::StderrAppend) => {
                                cmd.stderr(file);
                            }
                            None => {}
                        }
                    }
                    Err(_) => {
                        println!("{}: cannot open file for output redirection", filename);
                        return true;
                    }
                }
            }

            let status = cmd.status();
            match status {
                Ok(status) => {
                    if !status.success() {
                        //println!("{}: exited with status {}", command, status);
                    }
                }
                Err(e) => println!("{}: failed to execute: {}", command, e),
            }
            return true;
        } else {
            string_for_stderr = format!("{}: command not found\n", command);
        }
    }

    if filename.is_empty() {
        print!("{}", string_for_stdout);
        eprint!("{}", string_for_stderr);
    } else {
        let mut file_options = std::fs::OpenOptions::new();
        file_options.create(true).write(true);
        match redirect_to {
            Some(RedirectTo::Stdout) => { file_options.truncate(true); }
            Some(RedirectTo::Stderr) => { file_options.truncate(true); }
            Some(RedirectTo::StdoutAppend) => { file_options.append(true); }
            Some(RedirectTo::StderrAppend) => { file_options.append(true); }
            None => {}
        }

        match redirect_to {
            Some(RedirectTo::Stdout) | Some(RedirectTo::StdoutAppend) => {
                eprint!("{}", string_for_stderr);
                match file_options.open(filename) {
                    Ok(mut file) => {
                         write!(file, "{}", string_for_stdout).unwrap();
                    }
                    Err(_) => {
                        println!("{}: cannot open file for output redirection", filename);
                    }
                }
            }
            Some(RedirectTo::Stderr) | Some(RedirectTo::StderrAppend) => {
                print!("{}", string_for_stdout);
                match file_options.open(filename) {
                    Ok(mut file) => {
                         write!(file, "{}", string_for_stderr).unwrap();
                    }
                    Err(_) => {
                        println!("{}: cannot open file for output redirection", filename);
                    }
                }
            }
            _ => {
                println!("{}: invalid redirection", filename);
            }
        }
    }
    true
}

fn main() {
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();
        let mut console_input = String::new();
        let _ = io::stdin().read_line(&mut console_input);
        let (command, args, filename_opt, redirect_to) = parse_command(&console_input);
        let filename = filename_opt.as_deref().unwrap_or("");

        if !execute_command(&command, args, filename, redirect_to) {
            break;
        }
    }
}
