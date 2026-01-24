#[cfg(test)]
mod tests {
    use crate::{find_executable_in_path, parse_args, parse_command, RedirectTo};
    use std::fs::File;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_parse_args_simple() {
        let args = parse_args("hello world");
        assert_eq!(args, vec!["hello", "world"]);
    }

    #[test]
    fn test_parse_args_quoted() {
        let args = parse_args("'hello world'");
        assert_eq!(args, vec!["hello world"]);
    }

    #[test]
    fn test_parse_args_mixed() {
        let args = parse_args("echo 'hello world'");
        assert_eq!(args, vec!["echo", "hello world"]);
    }

    #[test]
    fn test_parse_args_adjacent_quotes() {
        // Based on analysis: 'hello''world' -> ["hello", "world"]
        // If the implementation changes to concatenate, this test should be updated.
        let args = parse_args("'hello''world'");
        assert_eq!(args, vec!["hello", "world"]);
    }

    #[test]
    fn test_parse_args_empty_and_spaces() {
        let args = parse_args("   hello   world   ");
        assert_eq!(args, vec!["hello", "world"]);
    }
    
    #[test]
    fn test_parse_args_inner_quotes() {
        // hello 'inner' world
        let args = parse_args("hello 'inner' world");
        assert_eq!(args, vec!["hello", "inner", "world"]);
    }

    // Helper to create a temp dir with an executable file
    // Returns (temp_dir_path, executable_path)
    fn setup_executable(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let mut dir = std::env::temp_dir();
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        dir.push(format!("cc_shell_test_{}", timestamp));
        std::fs::create_dir_all(&dir).expect("Failed to create temp dir");

        let file_path = dir.join(name);
        {
            let _file = File::create(&file_path).expect("Failed to create executable file");
            // Start of Unix specific code
            #[cfg(unix)]
            {
                let mut perms = _file.metadata().unwrap().permissions();
                use std::os::unix::fs::PermissionsExt;
                perms.set_mode(0o755);
                std::fs::set_permissions(&file_path, perms).expect("Failed to set permissions");
            }
            // End of Unix specific code
            // On Windows, files are generally executable if they have .exe extension or logic relies on other things.
            // But the tested function `find_executable_in_path` specifically checks `metadata.permissions().mode() & 0o111`.
            // This check is specific to Unix permissions model available via `std::os::unix`.
            // If running on Windows, this might fail or require `std::os::unix` to be available (e.g. Cygwin/MinGW).
            // However, since the main code uses it, we assume it's available.
        }
        
        (dir, file_path)
    }

    #[test]
    fn test_find_executable_found() {
        // Skip on non-unix if we can't set permissions, or rely on the function logic calling Unix APIs
        // The main code imports `std::os::unix::fs::PermissionsExt` unconditionally, so we assume we are in a unix-like env.
        
        let (dir, file_path) = setup_executable("my_exec");
        let path_env = dir.to_string_lossy();
        
        let result = find_executable_in_path("my_exec", Some(&path_env));
        
        assert_eq!(result, Some(file_path));
        
        // Cleanup
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_find_executable_not_found() {
        let (dir, _) = setup_executable("other_exec");
        let path_env = dir.to_string_lossy();
        
        let result = find_executable_in_path("non_existent", Some(&path_env));
        
        assert_eq!(result, None);
        
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_parse_command_simple() {
        let (cmd, args, filename, redirect) = parse_command("ls -l");
        assert_eq!(cmd, "ls");
        assert_eq!(args, vec!["-l"]);
        assert_eq!(filename, None);
        assert_eq!(redirect, None);
    }

    #[test]
    fn test_parse_command_with_quotes() {
        let (cmd, args, filename, redirect) = parse_command("echo 'hello world'");
        assert_eq!(cmd, "echo");
        assert_eq!(args, vec!["hello world"]);
        assert_eq!(filename, None);
        assert_eq!(redirect, None);
    }

    #[test]
    fn test_parse_command_redirect() {
        let (cmd, args, filename, redirect) = parse_command("echo hello > output.txt");
        assert_eq!(cmd, "echo");
        assert_eq!(args, vec!["hello"]);
        assert_eq!(filename, Some("output.txt".to_string()));
        assert_eq!(redirect, Some(RedirectTo::Stdout));
    }

    #[test]
    fn test_parse_command_redirect_explicit() {
        let (cmd, args, filename, redirect) = parse_command("cat file 1> out");
        assert_eq!(cmd, "cat");
        assert_eq!(args, vec!["file"]);
        assert_eq!(filename, Some("out".to_string()));
        assert_eq!(redirect, Some(RedirectTo::Stdout));
    }

    #[test]
    fn test_parse_command_redirect_quoted_filename() {
        let (cmd, args, filename, redirect) = parse_command("ls > 'my file'");
        assert_eq!(cmd, "ls");
        assert!(args.is_empty());
        assert_eq!(filename, Some("my file".to_string()));
        assert_eq!(redirect, Some(RedirectTo::Stdout));
    }

    #[test]
    fn test_parse_command_redirect_stderr() {
        let (cmd, args, filename, redirect) = parse_command("ls 2> error.log");
        assert_eq!(cmd, "ls");
        assert!(args.is_empty());
        assert_eq!(filename, Some("error.log".to_string()));
        assert_eq!(redirect, Some(RedirectTo::Stderr));
    }

    #[test]
    fn test_parse_command_redirect_stderr_with_args() {
        let (cmd, args, filename, redirect) = parse_command("grep foo bar 2> error.log");
        assert_eq!(cmd, "grep");
        assert_eq!(args, vec!["foo", "bar"]);
        assert_eq!(filename, Some("error.log".to_string()));
        assert_eq!(redirect, Some(RedirectTo::Stderr));
    }
}
