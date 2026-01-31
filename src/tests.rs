#[cfg(test)]
mod tests {
    use crate::{Shell, RedirectMode, MyHelper, CommandLine, Argument};
    use std::fs::File;
    use std::time::{SystemTime, UNIX_EPOCH};
    #[cfg(target_family = "unix")]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn test_completion_exact_match() {
        let helper = MyHelper {
            commands: vec!["echo".into(), "exit".into()],
            path_dirs: vec![],
        };
        let (start, matches) = helper.get_all_suggestions("echo", 4);
        assert_eq!(start, 0);
        assert_eq!(matches, vec!["echo "]);
    }

    #[test]
    fn test_completion_partial_match() {
        let helper = MyHelper {
            commands: vec!["echo".into(), "exit".into()],
            path_dirs: vec![],
        };
        let (start, matches) = helper.get_all_suggestions("ec", 2);
        assert_eq!(start, 0);
        assert_eq!(matches, vec!["echo "]);
    }

    #[test]
    fn test_completion_multiple_matches() {
        let helper = MyHelper {
            commands: vec!["echo".into(), "exit".into(), "echoloco".into()],
            path_dirs: vec![],
        };
        let (start, matches) = helper.get_all_suggestions("ec", 2);
        assert_eq!(start, 0);
        assert!(matches.contains(&"echo ".to_string()));
        assert!(matches.contains(&"echoloco ".to_string()));
        assert!(!matches.contains(&"exit ".to_string()));
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_completion_no_match() {
        let helper = MyHelper {
            commands: vec!["echo".into(), "exit".into()],
            path_dirs: vec![],
        };
        let (start, matches) = helper.get_all_suggestions("foo", 3);
        assert_eq!(start, 0);
        assert!(matches.is_empty());
    }

    #[test]
    fn test_completion_second_argument() {
        let helper = MyHelper {
            commands: vec!["echo".into(), "exit".into()],
            path_dirs: vec![],
        };
        let (start, matches) = helper.get_all_suggestions("sudo ec", 7);
        assert_eq!(start, 5);
        assert_eq!(matches, vec!["echo "]);
    }

    #[test]
    fn test_completion_executable_match() {
        let (temp_dir, _exec_path) = setup_executable("my_custom_exec");
        let helper = MyHelper {
            commands: vec!["echo".into()],
            path_dirs: vec![temp_dir.as_path().to_path_buf()],
        };
        let (start, matches) = helper.get_all_suggestions("my_c", 4);
        assert_eq!(start, 0);
        assert!(matches.contains(&"my_custom_exec ".to_string()));
        assert_eq!(matches.len(), 1); 

        let _ = std::fs::remove_dir_all(temp_dir);
    }
    
    #[test]
    fn test_completion_ech_partial() {
        let helper = MyHelper {
            commands: vec!["echo".into()],
            path_dirs: vec![],
        };
        let (start, matches) = helper.get_all_suggestions("ech", 3);
        assert_eq!(start, 0);
        assert_eq!(matches, vec!["echo "]);
    }

    #[test]
    fn test_parse_args_simple() {
        let cmd = CommandLine::parse("prog hello world");
        assert_eq!(cmd.args, vec![Argument::new("hello"), Argument::new("world")]);
    }

    #[test]
    fn test_parse_args_quoted() {
        let cmd = CommandLine::parse("prog 'hello world'");
        assert_eq!(cmd.args, vec![Argument::new("hello world")]);
    }

    #[test]
    fn test_parse_args_mixed() {
        let cmd = CommandLine::parse("echo 'hello world'");
        assert_eq!(cmd.args, vec![Argument::new("hello world")]);
    }

    #[test]
    fn test_parse_args_adjacent_quotes() {
        let cmd = CommandLine::parse("prog 'hello''world'");
        assert_eq!(cmd.args, vec![Argument::new("helloworld")]);
    }

    #[test]
    fn test_parse_args_empty_and_spaces() {
        let cmd = CommandLine::parse("prog    hello   world   ");
        assert_eq!(cmd.args, vec![Argument::new("hello"), Argument::new("world")]);
    }
    
    #[test]
    fn test_parse_args_inner_quotes() {
        let cmd = CommandLine::parse("prog hello 'inner' world");
        assert_eq!(cmd.args, vec![Argument::new("hello"), Argument::new("inner"), Argument::new("world")]);
    }

    #[test]
    fn test_parse_args_double_quotes() {
        let cmd = CommandLine::parse("echo \"hello world\"");
        assert_eq!(cmd.args, vec![Argument::new("hello world")]);
    }

    #[test]
    fn test_parse_command_simple() {
        let cmd_line = CommandLine::parse("ls -l");
        assert_eq!(cmd_line.command, "ls");
        assert_eq!(cmd_line.args, vec![Argument::new("-l")]);
        assert!(cmd_line.redirection.is_none());
    }
    
    #[test]
    fn test_parse_command_with_quotes() {
        let cmd_line = CommandLine::parse("echo 'hello world'");
        assert_eq!(cmd_line.command, "echo");
        assert_eq!(cmd_line.args, vec![Argument::new("hello world")]);
        assert!(cmd_line.redirection.is_none());
    }

    #[test]
    fn test_parse_command_redirect() {
        let cmd_line = CommandLine::parse("echo hello > output.txt");
        assert_eq!(cmd_line.command, "echo");
        assert_eq!(cmd_line.args, vec![Argument::new("hello")]);
        assert_eq!(cmd_line.redirection.clone().unwrap().target, "output.txt");
        assert_eq!(cmd_line.redirection.unwrap().mode, RedirectMode::Stdout);
    }
    
    #[test]
    fn test_parse_command_redirect_explicit() {
        let cmd_line = CommandLine::parse("cat file 1> out");
        assert_eq!(cmd_line.command, "cat");
        assert_eq!(cmd_line.args, vec![Argument::new("file")]);
        assert_eq!(cmd_line.redirection.clone().unwrap().target, "out");
        assert_eq!(cmd_line.redirection.unwrap().mode, RedirectMode::Stdout);
    }

    #[test]
    fn test_parse_command_redirect_quoted_filename() {
        let cmd_line = CommandLine::parse("ls > 'my file'");
        assert_eq!(cmd_line.command, "ls");
        assert!(cmd_line.args.is_empty());
        assert_eq!(cmd_line.redirection.clone().unwrap().target, "my file");
        assert_eq!(cmd_line.redirection.unwrap().mode, RedirectMode::Stdout);
    }

    #[test]
    fn test_parse_command_redirect_stderr() {
        let cmd_line = CommandLine::parse("ls 2> error.log");
        assert_eq!(cmd_line.command, "ls");
        assert!(cmd_line.args.is_empty());
        assert_eq!(cmd_line.redirection.clone().unwrap().target, "error.log");
        assert_eq!(cmd_line.redirection.unwrap().mode, RedirectMode::Stderr);
    }

    #[test]
    fn test_parse_command_redirect_stderr_with_args() {
        let cmd_line = CommandLine::parse("grep foo bar 2> error.log");
        assert_eq!(cmd_line.command, "grep");
        assert_eq!(cmd_line.args, vec![Argument::new("foo"), Argument::new("bar")]);
        assert_eq!(cmd_line.redirection.clone().unwrap().target, "error.log");
        assert_eq!(cmd_line.redirection.unwrap().mode, RedirectMode::Stderr);
    }

    #[test]
    fn test_parse_command_redirect_append() {
        let cmd_line = CommandLine::parse("ls >> out");
        assert_eq!(cmd_line.command, "ls");
        assert!(cmd_line.args.is_empty());
        assert_eq!(cmd_line.redirection.clone().unwrap().target, "out");
        assert_eq!(cmd_line.redirection.unwrap().mode, RedirectMode::StdoutAppend);
    }

    #[test]
    fn test_parse_command_redirect_stdout_append_explicit() {
        let cmd_line = CommandLine::parse("ls 1>> out");
        assert_eq!(cmd_line.command, "ls");
        assert!(cmd_line.args.is_empty());
        assert_eq!(cmd_line.redirection.clone().unwrap().target, "out");
        assert_eq!(cmd_line.redirection.unwrap().mode, RedirectMode::StdoutAppend);
    }

    #[test]
    fn test_parse_command_redirect_stderr_append() {
        let cmd_line = CommandLine::parse("ls 2>> out");
        assert_eq!(cmd_line.command, "ls");
        assert!(cmd_line.args.is_empty());
        assert_eq!(cmd_line.redirection.clone().unwrap().target, "out");
        assert_eq!(cmd_line.redirection.unwrap().mode, RedirectMode::StderrAppend);
    }

    // Helper to create a temp dir with an executable file
    fn setup_executable(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let mut dir = std::env::temp_dir();
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        dir.push(format!("cc_shell_test_{}", timestamp));
        std::fs::create_dir_all(&dir).expect("Failed to create temp dir");

        let file_path = dir.join(name);
        {
            let _file = File::create(&file_path).expect("Failed to create executable file");
            #[cfg(unix)]
            {
                let mut perms = _file.metadata().unwrap().permissions();
                use std::os::unix::fs::PermissionsExt;
                perms.set_mode(0o755);
                std::fs::set_permissions(&file_path, perms).expect("Failed to set permissions");
            }
        }
        
        (dir, file_path)
    }

    #[test]
    fn test_find_executable_found() {
        let (dir, file_path) = setup_executable("my_exec");
        
        let shell = Shell::with_settings(vec![dir.clone()]);
        let result = shell.find_executable_in_path("my_exec");
        
        assert_eq!(result, Some(file_path));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_find_executable_not_found() {
        let (dir, _) = setup_executable("other_exec");
        
        let shell = Shell::with_settings(vec![dir.clone()]);
        let result = shell.find_executable_in_path("non_existent");
        
        assert_eq!(result, None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_execute_builtin_echo_redirect_stdout() {
        let dir = std::env::temp_dir().join("shell_tests_stdout");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("out.txt");
        let file_path_str = file_path.to_str().unwrap();

        if file_path.exists() {
            std::fs::remove_file(&file_path).unwrap();
        }

        let shell = Shell::new();
        // echo hello > ...
        let cmd = CommandLine {
            command: "echo".to_string(),
            args: vec![Argument::new("hello")],
            redirection: Some(crate::Redirection { 
                target: file_path_str.to_string(), 
                mode: RedirectMode::Stdout 
            }),
        };
        shell.execute(cmd);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "hello\n");
    }

    #[test]
    fn test_execute_builtin_echo_redirect_append() {
        let dir = std::env::temp_dir().join("shell_tests_append");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("out.txt");
        let file_path_str = file_path.to_str().unwrap();

        if file_path.exists() {
             std::fs::remove_file(&file_path).unwrap();
        }
        
        let shell = Shell::new();
        let cmd1 = CommandLine {
            command: "echo".to_string(),
            args: vec![Argument::new("hello")],
            redirection: Some(crate::Redirection { target: file_path_str.to_string(), mode: RedirectMode::Stdout }),
        };
        shell.execute(cmd1);

        let cmd2 = CommandLine {
            command: "echo".to_string(),
            args: vec![Argument::new("world")],
            redirection: Some(crate::Redirection { target: file_path_str.to_string(), mode: RedirectMode::StdoutAppend }),
        };
        shell.execute(cmd2);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "hello\nworld\n");
    }

    #[test]
    fn test_execute_external_redirect_stdout() {
         let dir = std::env::temp_dir().join("shell_tests_ext_stdout");
         std::fs::create_dir_all(&dir).unwrap();
         let file_path = dir.join("out.txt");
         let file_path_str = file_path.to_str().unwrap();
         
         if file_path.exists() {
            std::fs::remove_file(&file_path).unwrap();
         }
         
         let shell = Shell::new();
         let cmd = CommandLine {
             command: "sh".to_string(),
             args: vec![Argument::new("-c"), Argument::new("echo external")],
             redirection: Some(crate::Redirection { target: file_path_str.to_string(), mode: RedirectMode::Stdout }),
         };
         shell.execute(cmd);
         
         let content = std::fs::read_to_string(&file_path).expect("File should exist");
         assert!(content.contains("external"));
    }

    #[test]
    fn test_execute_external_redirect_stderr() {
         let dir = std::env::temp_dir().join("shell_tests_ext_stderr");
         std::fs::create_dir_all(&dir).unwrap();
         let file_path = dir.join("err.txt");
         let file_path_str = file_path.to_str().unwrap();
         
         if file_path.exists() {
            std::fs::remove_file(&file_path).unwrap();
         }
         
         let shell = Shell::new();
         let cmd = CommandLine {
             command: "sh".to_string(),
             args: vec![Argument::new("-c"), Argument::new("echo failure >&2")],
             redirection: Some(crate::Redirection { target: file_path_str.to_string(), mode: RedirectMode::Stderr }),
         };
         shell.execute(cmd);
         
         let content = std::fs::read_to_string(&file_path).expect("File should exist");
         assert!(content.contains("failure"));
    }

    #[test]
    fn test_owl_scenario() {
         let rat_dir = std::env::temp_dir().join("rat_test");
         std::fs::create_dir_all(&rat_dir).unwrap();
         std::fs::write(rat_dir.join("banana"), "banana\n").unwrap();
         std::fs::write(rat_dir.join("grape"), "grape\n").unwrap();
         std::fs::write(rat_dir.join("pear"), "pear\n").unwrap();
         
         let owl_dir = std::env::temp_dir().join("owl_test");
         std::fs::create_dir_all(&owl_dir).unwrap();
         let bee_md = owl_dir.join("bee.md");
         if bee_md.exists() { std::fs::remove_file(&bee_md).unwrap(); }
         
         let rat_dir_str = rat_dir.to_str().unwrap();
         let bee_md_str = bee_md.to_str().unwrap();
         
         let shell = Shell::new();
         // ls -1 /tmp/rat >> /tmp/owl/bee.md
         let cmd = CommandLine {
             command: "ls".to_string(),
             args: vec![Argument::new("-1"), Argument::new(rat_dir_str)],
             redirection: Some(crate::Redirection { target: bee_md_str.to_string(), mode: RedirectMode::StdoutAppend }),
         };
         shell.execute(cmd);
         
         let content = std::fs::read_to_string(&bee_md).expect("ls output file should exist");
         assert!(content.contains("banana"));
         assert!(content.contains("grape"));
         assert!(content.contains("pear"));
         
         let fox_md = owl_dir.join("fox.md");
         let fox_md_str = fox_md.to_str().unwrap();
         if fox_md.exists() { std::fs::remove_file(&fox_md).unwrap(); }

         // echo 'Hello Maria' 1>> /tmp/owl/fox.md
         let cmd2 = CommandLine {
             command: "echo".to_string(),
             args: vec![Argument::new("Hello Maria")],
             redirection: Some(crate::Redirection { target: fox_md_str.to_string(), mode: RedirectMode::StdoutAppend }),
         };
         shell.execute(cmd2);
         
         let fox_content = std::fs::read_to_string(&fox_md).expect("echo output file should exist");
         assert_eq!(fox_content.trim(), "Hello Maria");
    }

    #[test]
    fn test_execute_builtin_pwd_redirect_stdout() {
        let dir = std::env::temp_dir().join("shell_tests_pwd");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("pwd_out.txt");
        let file_path_str = file_path.to_str().unwrap();

        if file_path.exists() {
            std::fs::remove_file(&file_path).unwrap();
        }

        let shell = Shell::new();
        let cmd = CommandLine {
            command: "pwd".to_string(),
            args: vec![],
            redirection: Some(crate::Redirection { target: file_path_str.to_string(), mode: RedirectMode::Stdout }),
        };
        shell.execute(cmd);

        let content = std::fs::read_to_string(&file_path).unwrap();
        let expected = std::env::current_dir().unwrap().to_string_lossy().to_string() + "\n";
        assert_eq!(content, expected);
    }

    #[test]
    fn test_execute_builtin_type_builtin() {
        let dir = std::env::temp_dir().join("shell_tests_type");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("type_out.txt");
        let file_path_str = file_path.to_str().unwrap();

        if file_path.exists() {
            std::fs::remove_file(&file_path).unwrap();
        }

        let shell = Shell::new();
        let cmd = CommandLine {
             command: "type".to_string(),
             args: vec![Argument::new("echo")],
             redirection: Some(crate::Redirection { target: file_path_str.to_string(), mode: RedirectMode::Stdout }),
        };
        shell.execute(cmd);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "echo is a shell builtin\n");
    }

    #[test]
    fn test_execute_builtin_type_not_found() {
        let out_dir = std::env::temp_dir().join("shell_tests_type_not");
        std::fs::create_dir_all(&out_dir).unwrap();
        let out_file = out_dir.join("type_out.txt");
        let out_file_str = out_file.to_str().unwrap();

        if out_file.exists() {
            std::fs::remove_file(&out_file).unwrap();
        }

        let shell = Shell::new();
        let cmd = CommandLine {
             command: "type".to_string(),
             args: vec![Argument::new("nonexistent")],
             redirection: Some(crate::Redirection { target: out_file_str.to_string(), mode: RedirectMode::Stdout }),
        };
        shell.execute(cmd);

        let content = std::fs::read_to_string(&out_file).unwrap();
        assert_eq!(content, "nonexistent: not found\n");

        std::fs::remove_dir_all(out_dir).unwrap();
    }

    #[test]
    fn test_execute_builtin_cd_relative() {
        let temp_base = std::env::temp_dir().join("test_cd_relative");
        std::fs::create_dir_all(&temp_base).unwrap();
        let sub_dir = temp_base.join("raspberry").join("orange");
        std::fs::create_dir_all(&sub_dir).unwrap();

        let original_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(&temp_base).unwrap();

        let shell = Shell::new();
        let cmd = CommandLine {
            command: "cd".to_string(),
            args: vec![Argument::new("./raspberry/orange")],
            redirection: None,
        };
        shell.execute(cmd);

        let new_cwd = std::env::current_dir().unwrap();
        assert_eq!(new_cwd, sub_dir);

        std::env::set_current_dir(&original_cwd).unwrap();
        std::fs::remove_dir_all(&temp_base).unwrap();
    }

    #[test]
    fn test_execute_builtin_cd_absolute_error() {
        let original_cwd = std::env::current_dir().unwrap();
        let shell = Shell::new();
        let cmd = CommandLine {
            command: "cd".to_string(),
            args: vec![Argument::new("/non-existing-directory")],
            redirection: None,
        };
        shell.execute(cmd);
        let new_cwd = std::env::current_dir().unwrap();
        assert_eq!(original_cwd, new_cwd); 
    }
}
