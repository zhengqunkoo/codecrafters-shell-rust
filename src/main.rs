#[allow(unused_imports)]
use std::env;

#[cfg(test)]
mod tests;

use std::io::Write;
#[cfg(target_family = "unix")]
use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, Mutex};

use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::{Context, Editor, Result, EventHandler, ConditionalEventHandler, Event, EventContext, RepeatCount, Cmd, KeyCode, KeyEvent, Modifiers};
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

pub fn find_longest_common_prefix(matches: &[String]) -> String {
    if matches.is_empty() {
        return String::new();
    }
    let mut prefix = matches[0].clone();
    if std::env::var("DEBUG").is_ok() {
        eprintln!("[DEBUG] Initial prefix: '{}'", prefix);
    }
    for m in &matches[1..] {
        let mut i = 0;
        let max = std::cmp::min(prefix.len(), m.len());
        while i < max && prefix.as_bytes()[i] == m.as_bytes()[i] {
            i += 1;
        }
        prefix.truncate(i);
        if std::env::var("DEBUG").is_ok() {
            eprintln!("[DEBUG] Truncated prefix after comparing with '{}': '{}'", m, prefix);
        }
    }
    prefix
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
                in_double_quote = false;
            } else if c == '\\' {
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
    pub path_dirs: Vec<std::path::PathBuf>,
}

impl MyHelper {
    pub fn get_all_suggestions(&self, line: &str, pos: usize) -> (usize, Vec<String>) {
        let (start, word_to_complete) = {
            let split_idx = line[..pos].rfind(' ').map(|i| i + 1).unwrap_or(0);
            (split_idx, &line[split_idx..pos])
        };

        let mut all_matches: Vec<String> = self
            .commands
            .iter()
            .filter(|cmd| cmd.starts_with(word_to_complete))
            .map(|cmd| format!("{} ", cmd)) // Add trailing space here
            .collect();

        let mut executable_matches = self.get_executable_suggestions(word_to_complete);
        all_matches.append(&mut executable_matches);

        all_matches.sort();
        all_matches.dedup();

        (start, all_matches)
    }

    /*
    Spec: Completing to Longest Common Prefix
    When multiple executables match the user's input, and some are prefixes of others, your shell should complete to the longest common prefix of all matches.

    For example, if these executables exist in PATH:

    xyz_foo
    xyz_foo_bar
    xyz_foo_bar_baz

    Pressing tab completes to the next common prefix of the remaining matches:

    # Note: The prompt lines below are displayed on the same line, and the user inserts '_' between each step.
    $ xyz_<TAB>
    $ xyz_foo_<TAB>
    $ xyz_foo_bar_<TAB>
    $ xyz_foo_bar_baz 

    There are no executable suggestions printed when the only remaining executables share a common prefix.
    */
    fn get_executable_suggestions(&self, word_to_complete: &str) -> Vec<String> {
        let mut suggestions = Vec::new();
        for path_dir in &self.path_dirs {
            // Skip if directory can't be read
            let Ok(entries) = std::fs::read_dir(path_dir) else { continue; };
            for entry in entries.flatten() {
                // Skip if filename is not valid UTF-8
                let file_name = entry.file_name();
                let Some(name_str) = file_name.to_str() else { continue; };
                // Skip if doesn't start with the word to complete
                if !name_str.starts_with(word_to_complete) { continue; }
                let full_path = path_dir.join(name_str);
                // Skip if can't get metadata
                let Ok(metadata) = std::fs::metadata(&full_path) else { continue; };
                // Check if it's an executable file
                let is_executable = if cfg!(target_family = "unix") {
                    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
                } else {
                    metadata.is_file()
                };
                if is_executable {
                    // Add trailing space to executable suggestions for consistency with builtins
                    suggestions.push(format!("{} ", name_str));
                }
            }
        }
        suggestions.sort();
        suggestions.dedup();
        suggestions
    }
}

// The Completer implementation for MyHelper is used by rustyline when the default completion
// mechanism is triggered (e.g., when Cmd::Complete is returned from an event handler).
// It provides completion candidates (suggestions) for the current input, and can also
// implement custom completion logic such as completing to the longest common prefix.
impl Completer for MyHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Pair>)> {
        let (start, matches) = self.get_all_suggestions(line, pos);
    
        let word_to_complete = &line[start..pos];
        let trimmed_matches: Vec<String> = matches.iter().map(|s| s.trim_end().to_string()).collect();
        let common_prefix = find_longest_common_prefix(&trimmed_matches);
        let add_space = matches.len() == 1 || common_prefix == word_to_complete;
    
        let pairs = matches
            .into_iter()
            .map(|cmd| {
                let replacement = if add_space {
                    format!("{} ", cmd.trim_end())
                } else {
                    cmd.trim_end().to_string()
                };
                Pair {
                    display: cmd.clone(),
                    replacement,
                }
            })
            .collect();
        
        Ok((start, pairs))
    }
}

struct TabState {
    consecutive_tabs: usize,
    last_line: String,
    last_pos: usize,
}

// MyTabHandler is a custom event handler for the Tab key.
// It controls the interactive tab completion experience, including:
// - Beeping on the first Tab press if there are multiple matches
// - Printing all suggestions on the second Tab press
// - Triggering completion (via Cmd::Complete) if there is only one match
// This handler is registered with rustyline to override the default Tab behavior.
struct MyTabHandler {
    state: Arc<Mutex<TabState>>, // Shared state across handler calls, protected by Mutex for thread safety.
    commands: Vec<String>, // List of builtin commands for completion.
    path_dirs: Vec<std::path::PathBuf>, // PATH directories to scan for executables.
}

impl MyTabHandler {
    // Gets suggestions for the current word at position in the line.
    // Returns a list of matching commands and executables.
    fn get_suggestions(&self, line: &str, pos: usize) -> Vec<String> {
        let (_, word_to_complete) = {
            let split_idx = line[..pos].rfind(' ').map(|i| i + 1).unwrap_or(0);
            (split_idx, &line[split_idx..pos])
        };

        let mut all_matches: Vec<String> = self
            .commands
            .iter()
            .filter(|cmd| cmd.starts_with(word_to_complete))
            .map(|cmd| cmd.to_string())
            .collect();

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
                                    all_matches.push(name_str.to_string());
                                }
                                #[cfg(target_family = "windows")]
                                if metadata.is_file() {
                                    all_matches.push(name_str.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        all_matches.sort();
        all_matches.dedup();
        all_matches
    }
}

// Implements ConditionalEventHandler to customize tab behavior.
// If one match, complete it. If multiple, beep on first tab, list on second.
impl ConditionalEventHandler for MyTabHandler {
    fn handle(&self, _event: &Event, _: RepeatCount, _: bool, ctx: &EventContext) -> Option<Cmd> {
        let current_line = ctx.line().to_string();
        let current_pos = ctx.pos();
        let matches = self.get_suggestions(&current_line, current_pos);

        // If exactly one match, perform completion.
        if matches.len() == 1 {
            return Some(Cmd::Complete);
        }

        let mut state = self.state.lock().unwrap();

        // Reset tab count if line or position changed.
        if current_line != state.last_line || current_pos != state.last_pos {
             state.consecutive_tabs = 0;
             state.last_line = current_line.clone();
             state.last_pos = current_pos;
        }

        // If no matches, beep and do nothing.
        if matches.is_empty() {
             print!("\x07"); // ASCII bell character for beep.
             std::io::stdout().flush().unwrap();
             return Some(Cmd::Noop);
        }

        state.consecutive_tabs += 1;

        // On first consecutive tab, complete user input to longer common prefix. If no longer common prefix found, beep.
        if state.consecutive_tabs == 1 {
            if std::env::var("DEBUG").is_ok() {
                eprintln!("[DEBUG] current_line: '{}', current_pos: {}", current_line, current_pos);
                eprintln!("[DEBUG] matches: {:?}", matches);
            }

            // Sub-spec: "All matches share a common prefix that's longer than the user's input.
            // Complete user's input to longest common prefix."
            let prefix = find_longest_common_prefix(&matches);
            if std::env::var("DEBUG").is_ok() {
                eprintln!("[DEBUG] Computed prefix: '{}'", prefix);
            }
            // Find the start of the word being completed
            let start = current_line[..current_pos].rfind(' ').map(|i| i + 1).unwrap_or(0);
            let word_len = current_pos - start;
            if prefix.len() > word_len {
                // Use rustyline's completer to handle buffer update and cursor positioning
                state.consecutive_tabs = 0;
                state.last_line = current_line.clone(); // Will be updated by completer
                state.last_pos = current_pos; // Will be updated by completer
                return Some(Cmd::Complete);
            } else {
                // If no longer common prefix found, beep.
                print!("\x07");
                std::io::stdout().flush().unwrap();
                Some(Cmd::Noop)
            }
        } else {
             // On second, print suggestions and reprint prompt.
             print!("\n");
             let joined = matches.join("  ");
             print!("{}", joined);
             print!("\n");
             print!("$ {}", current_line);
             std::io::stdout().flush().unwrap();
             Some(Cmd::Noop)
        }
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

    let commands = vec![
            "exit".into(), 
            "echo".into(), 
            "type".into(), 
            "pwd".into(), 
            "cd".into()
        ];

    let helper = MyHelper {
        commands: commands.clone(),
        path_dirs: path_dirs.clone(),
    };

    // Shared state for tracking tab presses.
    let tab_state = Arc::new(Mutex::new(TabState {
        consecutive_tabs: 0,
        last_line: String::new(),
        last_pos: 0,
    }));

    // Handler for tab events.
    let tab_handler = MyTabHandler {
        state: tab_state,
        commands: commands.clone(),
        path_dirs: path_dirs.clone(),
    };

    let mut rl = Editor::new()?;
    rl.set_helper(Some(helper));
    // Bind Tab key to our custom handler for advanced completion behavior.
    rl.bind_sequence(KeyEvent(KeyCode::Tab, Modifiers::NONE), EventHandler::Conditional(Box::new(tab_handler)));

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
