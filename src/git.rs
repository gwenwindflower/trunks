use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, ensure};

pub struct Worktree {
    pub root: PathBuf,
    pub git_directories: Vec<PathBuf>,
}

impl Worktree {
    pub fn discover(directory: &Path) -> Result<Self> {
        let root = git_path(directory, "--show-toplevel")?;
        let common = git_path(&root, "--git-common-dir")?;
        let private = git_path(&root, "--absolute-git-dir")?;
        let mut git_directories = vec![common];
        if git_directories[0] != private {
            git_directories.push(private);
        }
        Ok(Self {
            root,
            git_directories,
        })
    }
}

fn git_path(directory: &Path, flag: &str) -> Result<PathBuf> {
    let output = Command::new("git")
        .env_remove("GIT_DIR")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_WORK_TREE")
        .arg("-C")
        .arg(directory)
        .args(["rev-parse", "--path-format=absolute", flag])
        .output()
        .with_context(|| {
            format!(
                "cannot run git in {}; ensure Git is installed",
                directory.display()
            )
        })?;
    ensure!(
        output.status.success(),
        "git rev-parse {flag} in {} failed: {}",
        directory.display(),
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let stdout = String::from_utf8(output.stdout).context("Git returned a non-UTF-8 path")?;
    let path = stdout.strip_suffix('\n').unwrap_or(&stdout);
    ensure!(
        !path.is_empty(),
        "git rev-parse {flag} returned an empty path"
    );
    Path::new(path)
        .canonicalize()
        .with_context(|| format!("cannot resolve Git path {path}"))
}
