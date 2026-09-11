# trunks

Trunks is a narrow Rust CLI for Worktrunk creation hooks. Keep the permission change limited to the Git directories Git reports and the explicitly named permission profile.

- `src/git.rs` resolves paths through Git; never infer a private Git directory from a branch or worktree name.
- `src/config.rs` owns TOML edits and atomic writes. Preserve unrelated rules and comments, and keep unchanged configs byte-for-byte identical.
- The generated config is an overlay on an already-selected Codex permission profile. Do not add profile selection, baseline permissions, trust changes, or legacy sandbox migration implicitly.
- Use real disposable repositories for behavior tests in `tests/worktrees.rs`; isolate their Git identity and config from the developer's settings.
- `mise run check` runs prek, strict Clippy, and the Rust suite. Review task definitions before running them. Git hook installation is explicit through `mise run hooks:install`.
- The README documents the Codex prerequisite and Worktrunk hook ordering. Keep those contracts aligned with CLI help.
