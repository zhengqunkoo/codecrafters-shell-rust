---
name: rust-shell-dev
description: Expert guide for developing features, adding commands, and testing the CodeCrafters Rust shell.
---

# Rust Shell Development Actions

Use this skill when the user asks to modify the shell's behavior, add commands, or fix bugs in `codecrafters-shell-rust`.

## Capability: Add Builtin Command

When the user asks to create a new builtin command (e.g., `history`, `alias`, `export`):

1.  **Register Command**:
    You MUST add the command string to the `BUILTIN_COMMANDS` array in `src/main.rs`. This ensures `MyHelper` recognizes it for tab completion.
    ```rust
    // ...existing code...
    const BUILTIN_COMMANDS: &[&str] = &["exit", "echo", "type", "pwd", "cd", "YOUR_COMMAND"];
    ```

2.  **Implement Execution Logic**:
    In `src/main.rs`, locate the `execute_command` function.
    Add a new `match` arm for the command string.
    *   **Output**: Append standard output to the `string_for_stdout` variable. Ensure it ends with `\n` if appropriate.
    *   **Errors**: Append error messages to the `string_for_stderr` variable.
    *   **Return**: Return `true` to keep the shell running, or `false` to exit (only for `exit`).

    Example pattern:
    ```rust
    "your_command" => {
        // Implementation
        string_for_stdout.push_str("Output\n");
    },
    ```

## Capability: Fix Argument Parsing

If the user reports issues with quotes, spacing, or arguments:

1.  Direct your attention to `parse_args` in `src/main.rs`.
2.  **Constraint Checklist**:
    *   Single quotes `'` must preserve all literal characters inside.
    *   Double quotes `"` must preserve literals but allow for specific escapes if implemented (currently basic).
    *   Unquoted backslashes `\` should escape the next character.
    *   Argument splitting happens on whitespace *outside* of quotes.

## Capability: implementation Validation

When you have completed a change, verify it by:
1.  Running `cargo test` to ensure no regressions in `src/tests.rs`.
2.  If the change involves `MyHelper` (completion), check `test_completion_*` tests.
