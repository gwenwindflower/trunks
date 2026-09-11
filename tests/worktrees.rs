use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;
use toml_edit::DocumentMut;

struct Repo {
    _temp: TempDir,
    main: PathBuf,
    worktree: PathBuf,
}

impl Repo {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let main = temp.path().join("root repo");
        let worktree = temp.path().join("root repo.feature-branch");
        fs::create_dir(&main).unwrap();
        git(&main, &["init", "--quiet", "--initial-branch=main"]);
        git(
            &main,
            &[
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "Initial fixture",
            ],
        );
        git(
            &main,
            &[
                "worktree",
                "add",
                "--quiet",
                "-b",
                "feature/topic",
                worktree.to_str().unwrap(),
            ],
        );
        Self {
            _temp: temp,
            main: main.canonicalize().unwrap(),
            worktree: worktree.canonicalize().unwrap(),
        }
    }

    fn config(&self) -> PathBuf {
        self.worktree.join(".codex/config.toml")
    }

    fn write_config(&self, content: &str) {
        fs::create_dir_all(self.config().parent().unwrap()).unwrap();
        fs::write(self.config(), content).unwrap();
    }

    fn run(&self, args: &[&str]) -> Output {
        trunks(&self.worktree, args)
    }
}

fn git(directory: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(directory)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_AUTHOR_NAME", "Trunks test")
        .env("GIT_AUTHOR_EMAIL", "trunks@example.invalid")
        .env("GIT_COMMITTER_NAME", "Trunks test")
        .env("GIT_COMMITTER_EMAIL", "trunks@example.invalid")
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .trim_end_matches('\n')
        .to_owned()
}

fn trunks(directory: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_trunks"))
        .current_dir(directory)
        .args(args)
        .output()
        .unwrap()
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_permissions(content: &str, directory: &Path, profile: &str) {
    let document = content.parse::<DocumentMut>().unwrap();
    let rules = document["permissions"][profile]["filesystem"]
        .as_table_like()
        .unwrap();
    for flag in ["--git-common-dir", "--absolute-git-dir"] {
        let path = git(directory, &["rev-parse", "--path-format=absolute", flag]);
        let path = Path::new(&path).canonicalize().unwrap();
        assert_eq!(
            rules
                .get(path.to_str().unwrap())
                .and_then(toml_edit::Item::as_str),
            Some("write")
        );
    }
}

#[test]
fn creates_config_with_shared_and_private_git_permissions() {
    let repo = Repo::new();
    success(&repo.run(&[]));
    let content = fs::read_to_string(repo.config()).unwrap();
    assert_permissions(&content, &repo.worktree, "dev");
    let document = content.parse::<DocumentMut>().unwrap();
    assert_eq!(
        document["permissions"]["dev"]["filesystem"]
            .as_table()
            .unwrap()
            .len(),
        2
    );
    assert!(!repo.main.join(".codex").exists());
    assert!(document.get("default_permissions").is_none());
}

#[test]
fn preserves_comments_and_unrelated_settings_and_skips_repeat_writes() {
    let repo = Repo::new();
    let common = repo.main.join(".git");
    repo.write_config(&format!(
        "# Personal settings\nmodel = 'example-model'\ndefault_permissions = 'dev'\n\n[permissions.dev.filesystem]\n# Git metadata\n'{}' = 'read' # keep this comment\n'/unrelated' = 'deny'\n\n[permissions.other.network]\nenabled = false\n",
        common.display()
    ));
    success(&repo.run(&[]));
    let content = fs::read_to_string(repo.config()).unwrap();
    assert_permissions(&content, &repo.worktree, "dev");
    for preserved in [
        "# Personal settings\nmodel = 'example-model'",
        "default_permissions = 'dev'",
        "# Git metadata",
        "# keep this comment",
        "'/unrelated' = 'deny'",
        "[permissions.other.network]\nenabled = false",
    ] {
        assert!(
            content.contains(preserved),
            "missing {preserved:?} in {content}"
        );
    }
    let modified = fs::metadata(repo.config()).unwrap().modified().unwrap();
    let output = repo.run(&[]);
    success(&output);
    assert_eq!(fs::read_to_string(repo.config()).unwrap(), content);
    assert_eq!(
        fs::metadata(repo.config()).unwrap().modified().unwrap(),
        modified
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn updates_an_inline_filesystem_table_in_the_named_profile() {
    let repo = Repo::new();
    repo.write_config(
        "[permissions.'team.dev']\nfilesystem = { '/unrelated' = 'read' } # inline\n",
    );
    success(&repo.run(&["--profile", "team.dev"]));
    let content = fs::read_to_string(repo.config()).unwrap();
    assert_permissions(&content, &repo.worktree, "team.dev");
    let document = content.parse::<DocumentMut>().unwrap();
    assert!(document["permissions"].get("dev").is_none());
    assert!(content.contains("# inline"));
    assert_eq!(
        document["permissions"]["team.dev"]["filesystem"]["/unrelated"].as_str(),
        Some("read")
    );
}

#[test]
fn malformed_configs_and_conflicting_table_types_are_not_overwritten() {
    let repo = Repo::new();
    for content in [
        "[permissions.dev\n",
        "permissions = false\n",
        "[permissions]\ndev = 'bad'\n",
        "[permissions.dev]\nfilesystem = ['bad']\n",
    ] {
        repo.write_config(content);
        let output = repo.run(&[]);
        assert!(!output.status.success());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains("config.toml"), "{error}");
        assert_eq!(fs::read_to_string(repo.config()).unwrap(), content);
    }
}

#[test]
fn dry_run_prints_proposed_config_without_creating_or_modifying_files() {
    let repo = Repo::new();
    let output = repo.run(&["--dry-run"]);
    success(&output);
    assert_permissions(
        &String::from_utf8(output.stdout).unwrap(),
        &repo.worktree,
        "dev",
    );
    assert!(!repo.worktree.join(".codex").exists());
    repo.write_config("# Keep me\n");
    let output = repo.run(&["--dry-run"]);
    success(&output);
    assert_permissions(
        &String::from_utf8(output.stdout).unwrap(),
        &repo.worktree,
        "dev",
    );
    assert_eq!(fs::read_to_string(repo.config()).unwrap(), "# Keep me\n");
}

#[test]
fn directory_option_discovers_the_worktree_root_from_a_subdirectory() {
    let repo = Repo::new();
    let nested = repo.worktree.join("src/nested");
    fs::create_dir_all(&nested).unwrap();
    success(&trunks(&repo.main, &["-C", nested.to_str().unwrap()]));
    assert_permissions(
        &fs::read_to_string(repo.config()).unwrap(),
        &repo.worktree,
        "dev",
    );
    assert!(!nested.join(".codex").exists());
    assert!(!repo.main.join(".codex").exists());
}

#[test]
fn discovers_private_git_directory_after_a_worktree_move() {
    let repo = Repo::new();
    let moved = repo.worktree.with_file_name("different worktree name");
    git(
        &repo.main,
        &[
            "worktree",
            "move",
            repo.worktree.to_str().unwrap(),
            moved.to_str().unwrap(),
        ],
    );
    success(&trunks(&moved, &[]));
    let content = fs::read_to_string(moved.join(".codex/config.toml")).unwrap();
    assert_permissions(&content, &moved, "dev");
}

#[test]
fn primary_checkout_writes_a_single_git_directory_rule() {
    let repo = Repo::new();
    success(&trunks(&repo.main, &[]));
    let content = fs::read_to_string(repo.main.join(".codex/config.toml")).unwrap();
    assert_permissions(&content, &repo.main, "dev");
    let document = content.parse::<DocumentMut>().unwrap();
    assert_eq!(
        document["permissions"]["dev"]["filesystem"]
            .as_table()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn rejects_non_repositories_without_creating_config() {
    let temp = tempfile::tempdir().unwrap();
    let output = trunks(temp.path(), &[]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("git"));
    assert!(!temp.path().join(".codex").exists());
}

#[cfg(unix)]
#[test]
fn refuses_symlinked_config_files_and_directories() {
    use std::os::unix::fs::symlink;

    let repo = Repo::new();
    let source = repo.main.join("shared.toml");
    fs::write(&source, "# Shared config\n").unwrap();
    fs::create_dir(repo.worktree.join(".codex")).unwrap();
    symlink(&source, repo.config()).unwrap();
    let output = repo.run(&[]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("symlink"));
    assert_eq!(fs::read_to_string(&source).unwrap(), "# Shared config\n");
    assert!(fs::symlink_metadata(repo.config()).unwrap().is_symlink());

    let other = Repo::new();
    fs::create_dir(other.main.join(".codex")).unwrap();
    symlink(other.main.join(".codex"), other.worktree.join(".codex")).unwrap();
    let output = other.run(&[]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("symlink"));
    assert!(!other.main.join(".codex/config.toml").exists());
}

#[test]
fn directory_option_takes_precedence_over_inherited_git_repository_paths() {
    let repo = Repo::new();
    let output = Command::new(env!("CARGO_BIN_EXE_trunks"))
        .current_dir(&repo.main)
        .env("GIT_DIR", repo.main.join(".git"))
        .env("GIT_WORK_TREE", &repo.main)
        .args(["-C", repo.worktree.to_str().unwrap()])
        .output()
        .unwrap();
    success(&output);
    assert_permissions(
        &fs::read_to_string(repo.config()).unwrap(),
        &repo.worktree,
        "dev",
    );
    assert!(!repo.main.join(".codex").exists());
}

#[test]
fn bare_repository_is_rejected_without_creating_config() {
    let temp = tempfile::tempdir().unwrap();
    git(temp.path(), &["init", "--quiet", "--bare"]);
    let output = trunks(temp.path(), &[]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("work tree"));
    assert!(!temp.path().join(".codex").exists());
}

#[cfg(unix)]
#[test]
fn preserves_file_permissions_and_refuses_to_replace_read_only_config() {
    use std::os::unix::fs::PermissionsExt;

    let repo = Repo::new();
    repo.write_config("# Private settings\n");
    fs::set_permissions(repo.config(), fs::Permissions::from_mode(0o640)).unwrap();
    success(&repo.run(&[]));
    assert_eq!(
        fs::metadata(repo.config()).unwrap().permissions().mode() & 0o777,
        0o640
    );

    repo.write_config("# Read-only settings\n");
    fs::set_permissions(repo.config(), fs::Permissions::from_mode(0o440)).unwrap();
    let output = repo.run(&[]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("read-only"));
    assert_eq!(
        fs::read_to_string(repo.config()).unwrap(),
        "# Read-only settings\n"
    );
}

#[cfg(unix)]
#[test]
fn quotes_git_paths_with_toml_special_characters() {
    let repo = Repo::new();
    let linked = repo
        .worktree
        .with_file_name("worktree with a \"quote\" and ünicode");
    git(
        &repo.main,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "quoting",
            linked.to_str().unwrap(),
        ],
    );
    success(&trunks(&linked, &[]));
    assert_permissions(
        &fs::read_to_string(linked.join(".codex/config.toml")).unwrap(),
        &linked,
        "dev",
    );
}
