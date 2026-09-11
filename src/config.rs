use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use tempfile::NamedTempFile;
use toml_edit::{DocumentMut, Item, Table, TableLike, Value};

pub fn read(path: &Path) -> Result<Option<String>> {
    reject_symlink(path.parent().context("config path has no parent")?)?;
    reject_symlink(path)?;
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("cannot read {}", path.display())),
    }
}

fn reject_symlink(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => ensure!(
            !metadata.is_symlink(),
            "{} is a symlink; use a worktree-local .codex/config.toml",
            path.display()
        ),
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("cannot inspect {}", path.display()));
        }
    }
    Ok(())
}

pub fn grant_git_access(content: &str, profile: &str, directories: &[PathBuf]) -> Result<String> {
    let mut document = content.parse::<DocumentMut>().context("invalid TOML")?;
    let mut table: &mut dyn TableLike = document.as_table_mut();
    for name in ["permissions", profile, "filesystem"] {
        let item = table.entry(name).or_insert_with(|| {
            let mut table = Table::new();
            table.set_implicit(name != "filesystem");
            Item::Table(table)
        });
        table = item
            .as_table_like_mut()
            .with_context(|| format!("{name:?} must be a TOML table"))?;
    }
    for directory in directories {
        let key = directory
            .to_str()
            .context("Git path cannot be represented in UTF-8 TOML")?;
        let mut permission = Value::from("write");
        if let Some(existing) = table.get_mut(key) {
            ensure!(
                matches!(existing.as_str(), Some("read" | "write" | "deny")),
                "filesystem rule for {key:?} must be read, write, or deny"
            );
            if existing.as_str() == Some("write") {
                continue;
            }
            if let Some(value) = existing.as_value() {
                *permission.decor_mut() = value.decor().clone();
            }
            *existing = Item::Value(permission);
        } else {
            table.insert(key, Item::Value(permission));
        }
    }
    Ok(document.to_string())
}

pub fn write(path: &Path, content: &str) -> Result<()> {
    let parent = path.parent().context("config path has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("cannot create {}", parent.display()))?;
    let mut temporary = NamedTempFile::new_in(parent)
        .with_context(|| format!("cannot create a temporary config in {}", parent.display()))?;
    if let Ok(metadata) = fs::metadata(path) {
        ensure!(
            !metadata.permissions().readonly(),
            "{} is read-only",
            path.display()
        );
        temporary
            .as_file()
            .set_permissions(metadata.permissions())?;
    }
    temporary.write_all(content.as_bytes())?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .with_context(|| format!("cannot replace {}", path.display()))?;
    Ok(())
}
