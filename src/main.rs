mod config;
mod git;

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Parser;

#[derive(Parser)]
#[command(
    version,
    about,
    after_help = "Worktrunk creation hook (.config/wt.toml):\n  [pre-start]\n  trunks-agent-config = \"trunks\"\n\nThe named permission profile must be active in Codex. Trunks adds Git path\nrules without changing default_permissions or other sandbox settings."
)]
struct Cli {
    /// Discover the Git worktree containing this directory
    #[arg(short = 'C', long, default_value = ".", value_name = "PATH")]
    directory: PathBuf,

    /// Permission profile to extend in .codex/config.toml
    #[arg(short, long, default_value = "dev", value_name = "NAME", value_parser = profile_name)]
    profile: String,

    /// Print the resulting TOML without writing any files
    #[arg(long)]
    dry_run: bool,

    /// Show resolved paths and report unchanged configurations
    #[arg(short, long)]
    verbose: bool,
}

fn profile_name(name: &str) -> Result<String, String> {
    if name.trim().is_empty() || name.starts_with(':') {
        return Err("use a nonempty custom permission profile name, such as dev".into());
    }
    Ok(name.to_owned())
}

fn run(cli: &Cli) -> Result<()> {
    let worktree = git::Worktree::discover(&cli.directory)?;
    let path = worktree.root.join(".codex/config.toml");
    let original = config::read(&path)?;
    let rendered = config::grant_git_access(
        original.as_deref().unwrap_or_default(),
        &cli.profile,
        &worktree.git_directories,
    )
    .with_context(|| format!("cannot update {}", path.display()))?;

    if cli.verbose {
        eprintln!("trunks: config {}", path.display());
        eprintln!("trunks: permission profile {}", cli.profile);
        for directory in &worktree.git_directories {
            eprintln!("trunks: write {}", directory.display());
        }
    }

    if cli.dry_run {
        io::stdout().lock().write_all(rendered.as_bytes())?;
    } else if original.as_deref() != Some(rendered.as_str()) {
        config::write(&path, &rendered)
            .with_context(|| format!("cannot write {}", path.display()))?;
        eprintln!("trunks: updated {}", path.display());
    } else if cli.verbose {
        eprintln!("trunks: already configured");
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(&Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("trunks: {error:#}");
            ExitCode::FAILURE
        }
    }
}
