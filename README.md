# 🌲🐘 trunks

A tiny utility to prepare agent git permissions for a sibling worktrees pattern, optimized for [Worktrunk](https://worktrunk.dev/). Currently only affects Codex as other harnesses I use support the sibling pattern already.

<img width="1500" height="500" alt="trunks-flowers" src="https://github.com/user-attachments/assets/bf1f0716-969b-453a-911d-945b3be49213" />

## Using trunks

### What's the sibling pattern?

```bash
my-cool-repo/AGENTS.md
my-cool-repo.awesome-branch/AGENTS.md
my-cool-repo.fantastic-fix/AGENTS.md
```

### What does it do?

Run `trunks` inside a worktree. It asks Git for the shared repository directory and the current worktree's private Git directory, then creates or updates `.codex/config.toml` at the worktree root:

```toml
[permissions.dev.filesystem]
"/Users/winnie/projects/root-repo/.git" = "write"
"/Users/winnie/projects/root-repo/.git/worktrees/root-repo.feature-branch" = "write"
```

The shared directory holds objects, refs, and repository configuration. The private directory holds the worktree's index, HEAD, and operation state. Both receive explicit write rules because Codex protects Git metadata beneath workspace roots.

Codex can now run the git operations it needs for development work.

### Install

```sh
cargo install --path . --locked
```

Proper release and compiled binary for binstall coming soon.

### Worktrunk hook

Add this to `.config/wt.toml` in a project, or to your Worktrunk user config:

```toml
[pre-start]
trunks-agent-config = "trunks"
```

[Worktrunk's `pre-start` hook](https://worktrunk.dev/hook/) runs in the created worktree and finishes before `post-start` hooks or `--execute` launch Codex. If another creation hook copies `.codex/config.toml`, put that copy first and `trunks` second in a `[[pre-start]]` pipeline. Commands in one hook table run concurrently.

### Commands and options

```sh
trunks
trunks -C ../root-repo.feature --profile team-dev
trunks --dry-run
trunks --verbose
```

| Option | Behavior |
| --- | --- |
| `-C, --directory PATH` | Find the worktree containing `PATH`; defaults to the current directory |
| `-p, --profile NAME` | Update this permission profile; defaults to `dev` |
| `--dry-run` | Print the resulting TOML to stdout without creating or changing files |
| `-v, --verbose` | Print resolved paths, the selected profile, and unchanged status to stderr |

Normal runs report changes to stderr and stay silent when the file already has the required rules. Errors exit nonzero so the creation hook can stop.

### Codex setup

The named [permission profile](https://developers.openai.com/codex/permissions) must already be active in Codex. For example, a user config can select the `dev` profile and supply its workspace baseline:

```toml
default_permissions = "dev"

[permissions.dev]
extends = ":workspace"
```

Trunks adds only the Git filesystem rules to the project layer. It does not select a profile, alter its baseline, configure trust, or change approval and network settings. `--profile` names a permission profile under `[permissions]`, not a Codex launch profile. Project config must be trusted and loaded in a fresh Codex session. Legacy `sandbox_mode` settings take precedence over permission profiles unless managed policy selects the profile model.

### Behavior details

- Parses and edits TOML structurally, preserving comments and unrelated settings. Existing rules for the two exact Git paths become `write`; other rules remain intact.
- Resolves directories through Git, including moved worktrees, branch names with slashes, and paths with spaces. A primary checkout gets one rule because its shared and private Git directories are identical.
- Replaces changed files atomically and preserves file permissions. Repeat runs leave the file untouched.
- Rejects invalid TOML, conflicting table types, symlinked `.codex` directories or config files, and changes to read-only configs. A worktree needs its own config file.
- Leaves existing absolute-path entries in place. Generated paths are machine-specific; keep the config local or expect local changes when it is tracked.

The rules cover Git metadata access. More-specific restrictions, managed policy, credentials, and Git's own checks still apply.

## Development

Use your own Rust toolchain. `mise.toml` declares prek at `latest`. If prek is already installed globally, copy `mise.local.toml.example` to the gitignored `mise.local.toml`. Its `disable_tools` setting skips mise management of prek in this project, including declarations inherited from global mise config, so tasks use the installation on `PATH`.

`mise tasks` lists the commands. Review and trust the configuration, then run:

```sh
mise install
mise run hooks:install
mise run check
mise run build
```

The optimized binary is written to `target/release/trunks`. Tests create disposable repositories and sibling worktrees to exercise the CLI, Git discovery, and config editing together.
