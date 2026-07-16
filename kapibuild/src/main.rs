//! CLI tool for scaffolding kapi controller projects.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

// ---------------------------------------------------------------------------
// CLI structure
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "kapibuild", about = "Scaffolding tool for kapi controller projects", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scaffold a new kapi controller project.
    Init {
        /// Path for the new project directory.
        path: String,
    },
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let cli = Cli::parse();

    if let Err(err) = run(cli) {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

/// Top-level dispatcher.
fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Init { path } => cmd_init(path),
    }
}

// ---------------------------------------------------------------------------
// Command: init
// ---------------------------------------------------------------------------

/// Scaffold a new kapi controller project.
fn cmd_init(path: String) -> Result<()> {
    let project_dir = PathBuf::from(&path);

    let project_name = project_dir
        .file_name()
        .and_then(|n| n.to_str())
        .context("path has no valid directory name")?
        .to_string();

    if project_dir.exists() {
        anyhow::bail!("directory '{}' already exists", path);
    }

    // Create directory structure.
    create_dir(&project_dir)?;
    create_dir(&project_dir.join("src"))?;
    create_dir(&project_dir.join("src").join("controllers"))?;
    create_dir(&project_dir.join("api"))?;
    create_dir(&project_dir.join("schemas"))?;

    // Write files.
    write_cargo_toml(&project_dir, &project_name)?;
    write_kapifile(&project_dir)?;
    write_main_rs(&project_dir)?;
    write_controllers_mod(&project_dir)?;

    println!("Created kapi controller project '{}'", project_name);
    Ok(())
}

fn create_dir(path: &Path) -> Result<()> {
    std::fs::create_dir(path)
        .with_context(|| format!("failed to create directory '{}'", path.display()))
}

fn write_cargo_toml(project_dir: &Path, project_name: &str) -> Result<()> {
    let content = format!(
        r#"[package]
name = "{project_name}"
version = "0.1.0"
edition = "2024"

[dependencies]
kapi-core = "0.1"
kapi-client = "0.1"
kapi-controller = "0.1"
serde = {{ version = "1", features = ["derive"] }}
tokio = {{ version = "1", features = ["full"] }}
tracing = "0.1"
tracing-subscriber = "0.3"
async-trait = "0.1"
schemars = "0.8"
"#,
    );
    std::fs::write(project_dir.join("Cargo.toml"), content).context("failed to write Cargo.toml")
}

fn write_kapifile(project_dir: &Path) -> Result<()> {
    let content = "domain: kapi.io\nversion: v1\nresources: []\n";
    std::fs::write(project_dir.join("Kapifile"), content).context("failed to write Kapifile")
}

fn write_main_rs(project_dir: &Path) -> Result<()> {
    let content = r#"use kapi_client::client::KapiClient;
use kapi_controller::manager::Manager;

mod controllers;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt::init();
    
    let client = KapiClient::new("http://localhost:8080")?;
    let mut manager = Manager::new(client);
    
    // TODO: Register controllers here
    // manager.controller_for(key).reconciler(my_reconciler).register();
    
    manager.start().await?;
    
    Ok(())
}
"#;
    std::fs::write(project_dir.join("src").join("main.rs"), content)
        .context("failed to write src/main.rs")
}

fn write_controllers_mod(project_dir: &Path) -> Result<()> {
    let content = "//! Controllers for this project.\n";
    std::fs::write(project_dir.join("src").join("controllers").join("mod.rs"), content)
        .context("failed to write src/controllers/mod.rs")
}
