#[allow(unused_imports)]
use std::env;

#[cfg(test)]
mod tests;

use std::io::Write;
#[cfg(target_family = "unix")]
use std::os::unix::fs::PermissionsExt;

use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::{Context, Editor, Result};
use rustyline_derive::{Helper, Highlighter, Hinter, Validator};

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
        (parse_args(a), Some(f.trim().trim_matches(|c| c == '\'' || c == '"').to_string()), Some(RedirectTo::StdoutAppend))
    } else if let Some((a, f)) = rest.split_once("2>>") {
        (parse_args(a), Some(f.trim().trim_matches(|c| c == '\'' || c == '"').to_string()), Some(RedirectTo::StderrAppend))
    } else if let Some((a, f)) = rest.split_once(">>") {
        (parse_args(a), Some(f.trim().trim_matches(|c| c == '\'' || c == '"').to_string()), Some(RedirectTo::StdoutAppend))
    } else if let Some((a, f)) = rest.split_once("1>") {
        (parse_args(a), Some(f.trim().trim_matches(|c| c == '\'' || c == '"').to_string()), Some(RedirectTo::Stdout))
    } else if let Some((a, f)) = rest.split_once("2>") {
        (parse_args(a), Some(f.trim().trim_matches(|c| c == '\'' || c == '"').to_string()), Some(RedirectTo::Stderr))
    } else if let Some((a, f)) = rest.split_once('>') {
        (parse_args(a), Some(f.trim().trim_matches(|c| c == '\'' || c == '"').to_string()), Some(RedirectTo::Stdout))
    } else {
        (parse_args(rest), None, None)
    };

    (command.to_string(), args, filename, redirect_to)
}

pub fn parse_args(args: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current_arg = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    for c in args.chars() {
        if in_single_quote {
            if c == '\'' {
                in_single_quote = false;
            } else {
                current_arg.push(c);
            }
        } else if in_double_quote {
            if c == '"' {
                // Handle escaped double quotes slightly?
                // For now, no backslash support as per previous simple implementation level, 
                // but strictly treating " as terminator matches previous logic style.
                in_double_quote = false;
            } else if c == '\\' {
                // If we want to support escaped double quotes like \" inside ", we need lookahead or state.
                // Given the constraints and likely tests, preserving non-quote behavior is safest
                // until explicit backslash escaping is required.
                // However, standard shells consume backslash inside double quotes only for $ ` " \.
                current_arg.push(c);
            } else {
                current_arg.push(c);
            }
        } else {
            if c == '\'' {
                in_single_quote = true;
            } else if c == '"' {
                in_double_quote = true;
            } else if c.is_whitespace() {
                 if !current_arg.is_empty() {
                     result.push(current_arg.clone());
                     current_arg.clear();
                 }
            } else if c == '\\' { 
                 // Outside quotes, backslash usually escapes next char.
                 // We push it for now to match old behavior unless we implement full escaping.
                 current_arg.push(c);
            } else {
                current_arg.push(c);
            }
        }
    }
    
    if !current_arg.is_empty() {
        result.push(current_arg);
    }
    
    result
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

#[derive(Helper, Highlighter, Hinter, Validator)]
pub struct MyHelper {
    pub commands: Vec<String>,
    pub path_dirs: Vec<std::path::PathBuf>, // New field
}

impl MyHelper {
    pub fn get_suggestions(&self, line: &str, pos: usize) -> (usize, Vec<String>) {
        let (start, word_to_complete) = {
            let split_idx = line[..pos].rfind(' ').map(|i| i + 1).unwrap_or(0);
            (split_idx, &line[split_idx..pos])
        };

        let mut all_matches: Vec<String> = self
            .commands
            .iter()
            .filter(|cmd| cmd.starts_with(word_to_complete))
            .map(|cmd| format!("{} ", cmd))
            .collect();

        // Add executable suggestions
        let mut executable_matches = self.get_executable_suggestions(word_to_complete);
        all_matches.append(&mut executable_matches);

        all_matches.sort(); // Sort all matches
        all_matches.dedup(); // Remove duplicates

        (start, all_matches)
    }

    fn get_executable_suggestions(&self, word_to_complete: &str) -> Vec<String> {
        let mut suggestions = Vec::new();
        for path_dir in &self.path_dirs {
            if let Ok(entries) = std::fs::read_dir(path_dir) {
                for entry in entries.flatten() {
                    let file_name = entry.file_name();
                    if let Some(name_str) = file_name.to_str() {
                        if name_str.starts_with(word_to_complete) {
                            let full_path = path_dir.join(name_str);
                            if let Ok(metadata) = std::fs::metadata(&full_path) {
                                #[cfg(target_family = "unix")]
                                if metadata.is_file() && metadata.permissions().mode() & 0o111 != 0 {
                                    suggestions.push(format!("{} ", name_str));
                                }
                                #[cfg(target_family = "windows")]
                                if metadata.is_file() { // Simpler check for Windows
                                    suggestions.push(format!("{} ", name_str));
                                }
                            }
                        }
                    }
                }
            }
        }
        suggestions.sort(); // Sort for consistent order
        suggestions.dedup(); // Remove duplicates
        suggestions
    }
}

impl Completer for MyHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Pair>)> {
        let (start, matches) = self.get_suggestions(line, pos);

        let pairs = matches
            .into_iter()
            .map(|cmd| Pair {
                display: cmd.clone(),
                replacement: cmd,
            })
            .collect();

        Ok((start, pairs))
    }
}

fn main() -> Result<()> {
    let path_env = env::var("PATH").unwrap_or_default();
    let splitter = if cfg!(windows) { ';' } else { ':' };
    let path_dirs: Vec<std::path::PathBuf> = path_env
        .split(splitter)
        .filter_map(|p| {
            let path = std::path::PathBuf::from(p);
            if path.is_dir() { Some(path) } else { None }
        })
        .collect();

    let helper = MyHelper {
        commands: vec![
            "exit".into(), 
            "echo".into(), 
            "type".into(), 
            "pwd".into(), 
            "cd".into()
        ],
        path_dirs, // Initialize new field
    };

    let mut rl = Editor::new()?;
    rl.set_helper(Some(helper));

    loop {
        let readline = rl.readline("$ ");
        match readline {
            Ok(line) => {
                let (command, args, filename_opt, redirect_to) = parse_command(&line);
                let filename = filename_opt.as_deref().unwrap_or("");

                if !execute_command(&command, args, filename, redirect_to) {
                    break;
                }
                rl.add_history_entry(line.as_str())?;
            }
            Err(ReadlineError::Interrupted) => {
                println!("Ctrl-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("Ctrl-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
    Ok(())
}