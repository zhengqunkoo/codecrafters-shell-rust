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

### Refined Learnings

#### Context-Aware Action
- **Principle:** As an LLM, I inherently understand the context of tasks. My actions should reflect this by distinguishing between *generative* tasks (writing new code, which often involves replacement) and *additive/curatorial* tasks (like updating documentation or configuration files).
- **Behavior:** For documentation and config files (e.g., this `SKILL.md`), I will prioritize explicit checking for existing content and asking for your preferred modification method (append, overwrite, insert) before making changes.

#### Interruptibility & Pacing
- **Feedback Loop:** The critical challenge is ensuring you have sufficient time to intervene *before* irreversible actions are executed.
- **Protocol**:
    1.  **Propose Action:** I will explicitly state the command(s) or content changes I intend to make.
    2.  **Wait for Confirmation:** I will then *pause* and wait for your explicit approval ("yes," "proceed," "write file") before executing any destructive, history-altering, or non-trivial multi-step operations (e.g., `git commit`, `git push`, overwriting critical files, chaining multiple `replace` calls).
    3.  **Single-Step Execution:** Avoid chaining multiple irreversible commands when user review might be needed.
    4.  **Acknowledge Hard Stops:** "No!", "Stop!", or any alarm means I halt *all* current execution and re-evaluate my plan with your immediate feedback.

#### Transparency in Correction
- **Mistake Handling:** If I make a mistake, I will clearly explain the error and propose a solution. I will then await your confirmation before attempting to rectify it. This includes confirming whether the proposed fix aligns with your expectations, especially for "obvious" corrections.
