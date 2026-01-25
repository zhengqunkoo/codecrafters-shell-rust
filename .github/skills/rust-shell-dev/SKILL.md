---
name: rust-shell-dev
description: Expert guide for developing features, adding commands, and testing the CodeCrafters Rust shell.
---

# Rust Shell Development Actions

Use this skill when the user asks to modify the shell's behavior, add commands, or fix bugs in `codecrafters-shell-rust`.

**IMPORTANT**: Before executing any task, check `.github/skills/reflect/SKILL.md` for any "Learned Corrections" that might apply to your task.

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

---

# Rust Shell Development Skills & Reflections (from latest session)

## Technical Achievements

### 1. `rustyline` Integration
- **Objective:** Enhanced the CLI with interactive features (arrow key navigation, history).
- **Implementation:** Replaced `std::io::stdin` with `rustyline::Editor`.
- **Learnings**:
    - Integrating `rustyline` requires handling the `Editor` struct and its error types (`ReadlineError`).
    - Using `rustyline-derive` significantly reduces boilerplate for implementing helper traits (`Helper`, `Hinter`, `Highlighter`, `Validator`).

### 2. Tab Completion Implementation
- **Objective:** Provide intelligent suggestions for user input.
- **Implementation**:
    - Implemented the `Completer` trait for a custom `MyHelper` struct.
    - **Built-in Commands:** Filtered a static list of commands (echo, exit, etc.).
    - **Executable Discovery:** Implemented dynamic path searching:
        - Parsed the `PATH` environment variable.
        - Iterated through directories to identify executable files (checking permissions on Unix, file existence on Windows).
    - **UX Polish:** Appended a trailing space to unique completions to streamline the user experience (e.g., "echo " instead of "echo").

### 3. Bug Fixes & Refactoring
- **Quote Trimming:** Fixed a bug in `parse_command` where filenames in redirections were not correctly unquoted (e.g., `'file'` remaining `'file'`). Changed logic to `.trim_matches(|c| c == '\'' || c == '"')`.
- **Character Literals:** Fixed an "unterminated character literal" syntax error by correctly escaping backslashes (`'\\'`)
- **Testability:** Refactored completion logic into a public `get_suggestions` method on `MyHelper`, enabling direct unit testing without needing a full `rustyline` context.

## Workflow & Best Practices

### Git Hygiene
- **Safety:** Learned to prefer `git stash` over `git checkout` when handling uncommitted changes to avoid accidental data loss.
- **Clarity:** Emphasized the importance of accurate commit messages.
- **Atomic Commits:** Practiced separating feature implementation (completion) from fixes (bug corrections) where possible.

### Testing Strategy
- **Test-Driven Refactoring:** Wrote tests *before* finalizing feature implementation (e.g., for completion logic).
- **Regression Testing:** ran the full test suite (`cargo test`) locally before every commit to catch regressions early.
- **Mocking/Setup:** Used helper functions (like `setup_executable`) to create isolated test environments for file-system dependent tests.

## AI Collaboration & Workflow

### Context-Aware Modification
- **Judgment:** Distinguish between *generative* tasks (writing new code) and *additive* tasks (documenting, logging).
- **Validation:** When modifying documentation or configuration files (like this `SKILL.md`), explicitly check for existing content to avoid accidental overwrites.

### Proactive Transparency
- **Proposal vs. Action:** If a mistake is made (e.g., deleting user data), explain the cause and the proposed remedy *before* executing the fix, especially if it involves committing changes.
- **Communication:** When answering "Why?", explain the reasoning and propose the solution in the same turn, but wait for confirmation before acting.

### Interpreting User Feedback
- **Stop Signals:** "No" or expressions of alarm are hard stops. Halt tool execution immediately and await clarification.
- **Confirmation:** Do not assume the "next right step" after a stop signal; ask the user how they wish to proceed.