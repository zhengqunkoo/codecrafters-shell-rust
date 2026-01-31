#[allow(unused_imports)]
use std::env;

#[cfg(test)]
mod tests;

use std::io::Write;
#[cfg(target_family = "unix")]
use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, Mutex};
use std::path::PathBuf;

use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::{Context, Editor, Result, EventHandler, ConditionalEventHandler, Event, EventContext, RepeatCount, Cmd, KeyCode, KeyEvent, Modifiers};
use rustyline_derive::{Helper, Highlighter, Hinter, Validator};

// --- Domain Objects ---

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Argument {
    pub value: String,
}

impl Argument {
    pub fn new(value: impl Into<String>) -> Self {
        Self { value: value.into() }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum RedirectMode {
    Stdout,
    Stderr,
    StdoutAppend,
    StderrAppend,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Redirection {
    pub target: String,
    pub mode: RedirectMode,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CommandLine {
    pub command: String,
    pub args: Vec<Argument>,
    pub redirection: Option<Redirection>,
}

impl CommandLine {
    pub fn parse(input: &str) -> Self {
        let input = input.trim();
        let (command, rest) = input.split_once(' ').unwrap_or((input, ""));

        let (parsing_args_str, filename, mode) = 
            if let Some((a, f)) = rest.split_once("1>>") {
                (a, Some(f), Some(RedirectMode::StdoutAppend))
            } else if let Some((a, f)) = rest.split_once("2>>") {
                (a, Some(f), Some(RedirectMode::StderrAppend))
            } else if let Some((a, f)) = rest.split_once(">>") {
                (a, Some(f), Some(RedirectMode::StdoutAppend))
            } else if let Some((a, f)) = rest.split_once("1>") {
                (a, Some(f), Some(RedirectMode::Stdout))
            } else if let Some((a, f)) = rest.split_once("2>") {
                (a, Some(f), Some(RedirectMode::Stderr))
            } else if let Some((a, f)) = rest.split_once('>') {
                (a, Some(f), Some(RedirectMode::Stdout))
            } else {
                (rest, None, None)
            };

        let args = Self::parse_args_string(parsing_args_str);
        
        let redirection = if let (Some(f), Some(m)) = (filename, mode) {
             Some(Redirection {
                 target: f.trim().trim_matches(|c| c == '\'' || c == '"').to_string(),
                 mode: m,
             })
        } else {
            None
        };

        CommandLine {
            command: command.to_string(),
            args,
            redirection,
        }
    }

    fn parse_args_string(args: &str) -> Vec<Argument> {
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
                         result.push(Argument::new(current_arg.clone()));
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
            result.push(Argument::new(current_arg));
        }
        
        result
    }
}

// --- Command Interface ---

pub trait Command {
    fn name(&self) -> &str;
    fn execute(&self, args: &[Argument], redirection: Option<&Redirection>, shell: &Shell) -> bool;
}

pub struct ExitCommand;
impl Command for ExitCommand {
    fn name(&self) -> &str { "exit" }
    fn execute(&self, _args: &[Argument], _redirection: Option<&Redirection>, _shell: &Shell) -> bool {
        false
    }
}

pub struct EchoCommand;
impl Command for EchoCommand {
    fn name(&self) -> &str { "echo" }
    fn execute(&self, args: &[Argument], redirection: Option<&Redirection>, _shell: &Shell) -> bool {
        let output = args.iter().map(|a| a.value.as_str()).collect::<Vec<&str>>().join(" ") + "\n";
        CommandOutput::write(&output, "", redirection);
        true
    }
}

pub struct TypeCommand;
impl Command for TypeCommand {
    fn name(&self) -> &str { "type" }
    fn execute(&self, args: &[Argument], redirection: Option<&Redirection>, shell: &Shell) -> bool {
        let mut stdout = String::new();
        for arg in args {
            let name = &arg.value;
            if shell.is_builtin(name) {
                stdout.push_str(&format!("{} is a shell builtin\n", name));
            } else if let Some(path) = shell.find_executable_in_path(name) {
                stdout.push_str(&format!("{} is {}\n", name, path.display()));
            } else {
                stdout.push_str(&format!("{}: not found\n", name));
            }
        }
        CommandOutput::write(&stdout, "", redirection);
        true
    }
}

pub struct PwdCommand;
impl Command for PwdCommand {
    fn name(&self) -> &str { "pwd" }
    fn execute(&self, _args: &[Argument], redirection: Option<&Redirection>, _shell: &Shell) -> bool {
        match env::current_dir() {
            Ok(path) => CommandOutput::write(&(path.display().to_string() + "\n"), "", redirection),
            Err(e) => CommandOutput::write("", &format!("pwd: error retrieving current directory: {}\n", e), redirection),
        }
        true
    }
}

pub struct CdCommand;
impl Command for CdCommand {
    fn name(&self) -> &str { "cd" }
    fn execute(&self, args: &[Argument], _redirection: Option<&Redirection>, _shell: &Shell) -> bool {
        if args.len() > 1 {
            eprint!("cd: too many arguments\n");
        } else {
            let target_dir = if args.is_empty() || args[0].value == "~" {
                env::var("HOME").unwrap_or_else(|_| String::new())
            } else {
                args[0].value.clone()
            };
            if let Err(_) = env::set_current_dir(&target_dir) {
                eprint!("cd: {}: No such file or directory\n", target_dir);
            }
        }
        true
    }
}

pub struct ExternalCommand {
    name: String,
}

impl Command for ExternalCommand {
    fn name(&self) -> &str { &self.name }
    fn execute(&self, args: &[Argument], redirection: Option<&Redirection>, shell: &Shell) -> bool {
        if let Some(full_path) = shell.find_executable_in_path(&self.name) {
            let executable = full_path.file_name().unwrap();
            let mut cmd = std::process::Command::new(executable);
            cmd.args(args.iter().map(|a| &a.value));

            if let Some(r) = redirection {
                let mut fs_open_options = std::fs::OpenOptions::new();
                fs_open_options.create(true).write(true);
                match r.mode {
                    RedirectMode::Stdout | RedirectMode::Stderr => { fs_open_options.truncate(true); }
                    RedirectMode::StdoutAppend | RedirectMode::StderrAppend => { fs_open_options.append(true); }
                }

                match fs_open_options.open(&r.target) {
                    Ok(file) => {
                        match r.mode {
                            RedirectMode::Stdout | RedirectMode::StdoutAppend => { cmd.stdout(file); }
                            RedirectMode::Stderr | RedirectMode::StderrAppend => { cmd.stderr(file); }
                        }
                    }
                    Err(_) => {
                        println!("{}: cannot open file for output redirection", r.target);
                        return true;
                    }
                }
            }

            match cmd.status() {
                Ok(_) => {}, 
                Err(e) => println!("{}: failed to execute: {}", self.name, e),
            }
        } else {
            eprint!("{}: command not found\n", self.name); 
        }
        true
    }
}

// Helper for output handling
struct CommandOutput;
impl CommandOutput {
    fn write(stdout: &str, stderr: &str, redirection: Option<&Redirection>) {
        if let Some(r) = redirection {
             let mut options = std::fs::OpenOptions::new();
             options.create(true).write(true);
             match r.mode {
                 RedirectMode::Stdout | RedirectMode::Stderr => { options.truncate(true); }
                 RedirectMode::StdoutAppend | RedirectMode::StderrAppend => { options.append(true); }
             }

             match r.mode {
                 RedirectMode::Stdout | RedirectMode::StdoutAppend => {
                     eprint!("{}", stderr);
                     if let Ok(mut f) = options.open(&r.target) {
                         let _ = write!(f, "{}", stdout);
                     } else {
                         println!("{}: cannot open file for output redirection", r.target);
                     }
                 }
                 RedirectMode::Stderr | RedirectMode::StderrAppend => {
                     print!("{}", stdout);
                      if let Ok(mut f) = options.open(&r.target) {
                         let _ = write!(f, "{}", stderr);
                     } else {
                         println!("{}: cannot open file for output redirection", r.target);
                     }
                 }
             }
        } else {
            print!("{}", stdout);
            eprint!("{}", stderr);
        }
    }
}

// --- Shell ---

pub struct Shell {
    pub builtins: Vec<Box<dyn Command>>,
    pub path_dirs: Vec<PathBuf>,
}

impl Shell {
    pub fn new() -> Self {
        let path_env = env::var("PATH").unwrap_or_default();
        let splitter = if cfg!(windows) { ';' } else { ':' };
        let path_dirs: Vec<PathBuf> = path_env
            .split(splitter)
            .filter_map(|p| {
                let path = PathBuf::from(p);
                if path.is_dir() { Some(path) } else { None }
            })
            .collect();

        let builtins: Vec<Box<dyn Command>> = vec![
            Box::new(ExitCommand), 
            Box::new(EchoCommand), 
            Box::new(TypeCommand), 
            Box::new(PwdCommand), 
            Box::new(CdCommand)
        ];

        Shell {
            builtins,
            path_dirs,
        }
    }
    
    pub fn with_settings(path_dirs: Vec<PathBuf>) -> Self {
        Shell { builtins: vec![], path_dirs }
    }

    pub fn is_builtin(&self, name: &str) -> bool {
        self.builtins.iter().any(|c| c.name() == name)
    }

    pub fn find_executable_in_path(&self, executable: &str) -> Option<PathBuf> {
        for path_dir in &self.path_dirs {
            let full_path = path_dir.join(executable);
            if let Ok(_metadata) = std::fs::metadata(&full_path) {
                #[cfg(target_family = "unix")]
                if _metadata.permissions().mode() & 0o111 != 0 {
                    return Some(full_path);
                }
                #[cfg(target_family = "windows")]
                return Some(full_path);
            }
        }
        None
    }

    pub fn execute(&self, cmd_line: CommandLine) -> bool {
        if cmd_line.command.is_empty() { return true; }
        
        if let Some(cmd) = self.builtins.iter().find(|c| c.name() == cmd_line.command) {
            return cmd.execute(&cmd_line.args, cmd_line.redirection.as_ref(), self);
        }
        
        let ext_cmd = ExternalCommand { name: cmd_line.command.clone() };
        ext_cmd.execute(&cmd_line.args, cmd_line.redirection.as_ref(), self)
    }

    pub fn run(&mut self) -> Result<()> {
        let helper = MyHelper {
            commands: self.builtins.iter().map(|c| c.name().to_string()).collect(),
            path_dirs: self.path_dirs.clone(),
        };

        let tab_state = Arc::new(Mutex::new(TabState {
            consecutive_tabs: 0,
            last_line: String::new(),
            last_pos: 0,
        }));

        let tab_handler = MyTabHandler {
            state: tab_state,
            commands: self.builtins.iter().map(|c| c.name().to_string()).collect(),
            path_dirs: self.path_dirs.clone(),
        };

        let mut rl = Editor::new()?;
        rl.set_helper(Some(helper));
        rl.bind_sequence(KeyEvent(KeyCode::Tab, Modifiers::NONE), EventHandler::Conditional(Box::new(tab_handler)));

        loop {
            let readline = rl.readline("$ ");
            match readline {
                Ok(line) => {
                    let cmd_line = CommandLine::parse(&line);
                    if !self.execute(cmd_line) {
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
            .map(|cmd| format!("{} ", cmd))
            .collect();

        let mut executable_matches = self.get_executable_suggestions(word_to_complete);
        all_matches.append(&mut executable_matches);

        all_matches.sort();
        all_matches.dedup();

        (start, all_matches)
    }

    fn get_executable_suggestions(&self, word_to_complete: &str) -> Vec<String> {
        let mut suggestions = Vec::new();
        for path_dir in &self.path_dirs {
            let Ok(entries) = std::fs::read_dir(path_dir) else { continue; };
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let Some(name_str) = file_name.to_str() else { continue; };
                if !name_str.starts_with(word_to_complete) { continue; }
                let full_path = path_dir.join(name_str);
                let Ok(metadata) = std::fs::metadata(&full_path) else { continue; };
                let is_executable = if cfg!(target_family = "unix") {
                    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
                } else {
                    metadata.is_file()
                };
                if is_executable {
                    suggestions.push(format!("{} ", name_str));
                }
            }
        }
        suggestions.sort();
        suggestions.dedup();
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

struct MyTabHandler {
    state: Arc<Mutex<TabState>>,
    commands: Vec<String>,
    path_dirs: Vec<std::path::PathBuf>,
}

impl MyTabHandler {
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

impl ConditionalEventHandler for MyTabHandler {
    fn handle(&self, _event: &Event, _: RepeatCount, _: bool, ctx: &EventContext) -> Option<Cmd> {
        let current_line = ctx.line().to_string();
        let current_pos = ctx.pos();
        let matches = self.get_suggestions(&current_line, current_pos);

        if matches.len() == 1 {
            return Some(Cmd::Complete);
        }

        let mut state = self.state.lock().unwrap();

        if current_line != state.last_line || current_pos != state.last_pos {
             state.consecutive_tabs = 0;
             state.last_line = current_line.clone();
             state.last_pos = current_pos;
        }

        if matches.is_empty() {
             print!("\x07");
             std::io::stdout().flush().unwrap();
             return Some(Cmd::Noop);
        }

        state.consecutive_tabs += 1;

        if state.consecutive_tabs == 1 {
            let prefix = find_longest_common_prefix(&matches);
            let start = current_line[..current_pos].rfind(' ').map(|i| i + 1).unwrap_or(0);
            let word_len = current_pos - start;
            if prefix.len() > word_len {
                state.consecutive_tabs = 0;
                state.last_line = current_line.clone();
                state.last_pos = current_pos;
                return Some(Cmd::Complete);
            } else {
                print!("\x07");
                std::io::stdout().flush().unwrap();
                Some(Cmd::Noop)
            }
        } else {
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
    let mut shell = Shell::new();
    shell.run()
}
