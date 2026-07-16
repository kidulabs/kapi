//! CLI tool for scaffolding kapi controller projects.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use serde::{Deserialize, Serialize};

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
    /// Manage API resources in an existing project.
    #[command(subcommand)]
    Api(ApiCommands),
}

#[derive(Subcommand)]
enum ApiCommands {
    /// Create a new API resource skeleton.
    Create(ApiCreateArgs),
}

#[derive(Args, Debug)]
struct ApiCreateArgs {
    /// API group (e.g. example.io).
    #[arg(long, required = true)]
    group: String,

    /// API version (e.g. v1).
    #[arg(long, required = true)]
    version: String,

    /// Resource kind (e.g. Widget).
    #[arg(long, required = true)]
    kind: String,

    /// Resource scope: Namespaced or Cluster.
    #[arg(long, default_value = "Namespaced")]
    scope: String,

    /// Generate a status sub-resource struct.
    #[arg(long, default_value_t = false)]
    status: bool,
}

// ---------------------------------------------------------------------------
// Kapifile data model
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
struct Kapifile {
    domain: String,
    version: String,
    #[serde(default)]
    resources: Vec<KapifileResource>,
}

#[derive(Debug, Serialize, Deserialize)]
struct KapifileResource {
    kind: String,
    version: String,
    scope: String,
    #[serde(default)]
    has_status: bool,
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
        Commands::Api(api_cmd) => match api_cmd {
            ApiCommands::Create(args) => cmd_api_create(args),
        },
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

// ---------------------------------------------------------------------------
// Command: api create
// ---------------------------------------------------------------------------

/// Create a new API resource skeleton.
fn cmd_api_create(args: ApiCreateArgs) -> Result<()> {
    // Validate scope.
    let scope = validate_scope(&args.scope)?;

    // Find project root (where Kapifile lives).
    let project_root = find_project_root()?;

    // Build file path: api/<group>/<version>/<kind>.rs  (kind lowercased).
    let kind_lower = args.kind.to_lowercase();
    let api_dir = project_root.join("api").join(&args.group).join(&args.version);
    let api_file = api_dir.join(format!("{kind_lower}.rs"));

    // Prevent overwriting an existing kind.
    if api_file.exists() {
        anyhow::bail!("API resource '{}' already exists at '{}'", args.kind, api_file.display(),);
    }

    // Create directories.
    std::fs::create_dir_all(&api_dir)
        .with_context(|| format!("failed to create directory '{}'", api_dir.display()))?;

    // Generate skeleton file.
    let content = generate_skeleton(&args.kind, &args.group, &args.version, &scope, args.status);
    std::fs::write(&api_file, &content)
        .with_context(|| format!("failed to write '{}'", api_file.display()))?;

    // Update Kapifile.
    update_kapifile(&project_root, &args.kind, &args.version, &scope, args.status)?;

    println!(
        "Created API resource '{}' ({}/{}/{})",
        args.kind, args.group, args.version, kind_lower
    );

    Ok(())
}

/// Validate scope string, returning a canonical form.
fn validate_scope(scope: &str) -> Result<String> {
    match scope {
        "Namespaced" | "Cluster" => Ok(scope.to_string()),
        other => anyhow::bail!("invalid scope '{other}': must be 'Namespaced' or 'Cluster'"),
    }
}

/// Walk up from cwd looking for a Kapifile; returns its parent directory.
fn find_project_root() -> Result<PathBuf> {
    let cwd = std::env::current_dir().context("failed to get current directory")?;
    let mut dir = Some(cwd.as_path());

    while let Some(d) = dir {
        if d.join("Kapifile").exists() {
            return Ok(d.to_path_buf());
        }
        dir = d.parent();
    }

    anyhow::bail!(
        "no Kapifile found in current or parent directories — are you in a kapi project?"
    );
}

/// Generate the Rust source for an API resource skeleton.
fn generate_skeleton(
    kind: &str,
    group: &str,
    version: &str,
    scope: &str,
    has_status: bool,
) -> String {
    let kapi_attr = format!(
        r#"#[kapi(group = "{group}", version = "{version}", kind = "{kind}", scope = "{scope}")]"#
    );

    let mut out = String::new();
    out.push_str(
        "use kapi_controller::KapiResource;\n\
         use schemars::JsonSchema;\n\
         use serde::{Deserialize, Serialize};\n\
         \n\
         #[derive(Debug, Clone, KapiResource, Serialize, Deserialize, JsonSchema)]\n",
    );
    out.push_str(&kapi_attr);
    out.push('\n');
    out.push_str(&format!(
        "pub struct {kind}Spec {{\n\
         \n    // TODO: Add your spec fields here\n\
         \n    pub field1: String,\n    pub field2: i32,\n}}\n"
    ));

    if has_status {
        out.push_str(
            "\n\
             #[derive(Debug, Clone, KapiResource, Serialize, Deserialize, JsonSchema)]\n",
        );
        out.push_str(&kapi_attr);
        out.push('\n');
        out.push_str(&format!(
            "pub struct {kind}Status {{\n\
             \n    // TODO: Add your status fields here\n\
             \n    pub ready: bool,\n    pub message: String,\n}}\n"
        ));
    }

    out
}

/// Parse the Kapifile, add a resource entry, and write it back.
fn update_kapifile(
    project_root: &Path,
    kind: &str,
    version: &str,
    scope: &str,
    has_status: bool,
) -> Result<()> {
    let kapifile_path = project_root.join("Kapifile");
    let content = std::fs::read_to_string(&kapifile_path).context("failed to read Kapifile")?;

    let mut kapifile: Kapifile =
        serde_yaml::from_str(&content).context("failed to parse Kapifile")?;

    // Check for duplicate resource entry.
    if kapifile.resources.iter().any(|r| r.kind == kind) {
        anyhow::bail!("resource '{}' is already registered in Kapifile", kind);
    }

    kapifile.resources.push(KapifileResource {
        kind: kind.to_string(),
        version: version.to_string(),
        scope: scope.to_string(),
        has_status,
    });

    let updated = serde_yaml::to_string(&kapifile).context("failed to serialize Kapifile")?;
    std::fs::write(&kapifile_path, &updated).context("failed to write Kapifile")?;

    Ok(())
}
