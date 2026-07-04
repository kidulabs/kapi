//! CLI tool for the kapi API server.
//!
//! Supports the standard CRUD operations plus watch, status sub-resources,
//! and shell completions.

#![deny(rust_2024_compatibility)]

use std::collections::HashMap;
use std::path::Path;

use clap::{Parser, Subcommand, ValueEnum};
use kapi_client::client::KapiClient;
use kapi_client::error::ClientError;
use kapi_client::{
    ContinueToken, LabelSelector, ListOptions, ListResponse, ObjectMeta, ResourceKey, StoredObject,
    WatchFilter,
};
use serde::Deserialize;
use serde_json::Value;

use futures_util::StreamExt;

mod config;
mod error;
mod output;
mod resolver;

use config::Config;
use error::CliError;
use output::{format_json, format_table, format_table_list, format_watch_event, format_yaml};
use resolver::SchemaResolver;

// ---------------------------------------------------------------------------
// CLI structure
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "kapi", about = "CLI for kapi API server", version)]
struct Cli {
    /// Target namespace (defaults to "default" for namespaced kinds).
    #[arg(short, long, global = true)]
    namespace: Option<String>,

    /// Watch/list across all namespaces.
    #[arg(short = 'A', long, global = true)]
    all_namespaces: bool,

    /// Output format.
    #[arg(short, long, global = true, default_value = "table")]
    output: OutputFormat,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Edit a resource in your editor (like kubectl edit).
    Edit {
        /// Resource kind (supports `group/kind` syntax).
        kind: String,
        /// Resource name.
        name: String,
    },
    /// Get one or more resources.
    Get {
        /// Resource kind (supports `group/kind` syntax).
        kind: String,
        /// Resource name (omit to list all).
        name: Option<String>,
        /// Label selector (e.g. `app=nginx,env=prod`).
        #[arg(short = 'l')]
        label_selector: Option<String>,
        /// Limit the number of results (no auto-pagination).
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Create or update a resource from a file.
    Apply {
        /// Path to the manifest file (JSON or YAML).
        #[arg(short = 'f')]
        file: String,
    },
    /// Delete a resource by name.
    Delete {
        /// Resource kind.
        kind: String,
        /// Resource name.
        name: String,
    },
    /// Watch for events on a resource kind.
    Watch {
        /// Resource kind (supports `group/kind` syntax).
        kind: String,
        /// Label selector to filter watched events.
        #[arg(short = 'l')]
        label_selector: Option<String>,
    },
    /// Get or update the status sub-resource.
    #[command(subcommand)]
    Status(StatusCommands),
    /// Generate shell completion scripts.
    Completions {
        /// Target shell.
        shell: clap_complete::Shell,
    },
}

#[derive(Subcommand)]
enum StatusCommands {
    /// Get the status of a resource.
    Get {
        /// Resource kind.
        kind: String,
        /// Resource name.
        name: String,
    },
    /// Apply a status update from a file.
    Apply {
        /// Resource kind.
        kind: String,
        /// Resource name.
        name: String,
        /// Path to the status file (JSON or YAML).
        #[arg(short = 'f')]
        file: String,
    },
}

#[derive(Clone, ValueEnum)]
enum OutputFormat {
    Table,
    Json,
    Yaml,
}

// ---------------------------------------------------------------------------
// Apply manifest structure
// ---------------------------------------------------------------------------

/// Structure of a kubectl-style apply manifest.
#[derive(Debug, Deserialize)]
struct ApplyManifest {
    /// Resource kind (e.g. `"Widget"`).
    kind: String,
    /// API version in `group/version` format (e.g. `"example.io/v1"`).
    #[serde(rename = "apiVersion")]
    api_version: String,
    /// Standard object metadata.
    metadata: ApplyMetadata,
    /// Resource spec (the desired state).
    #[serde(default)]
    spec: Value,
    /// System metadata (read-only, ignored during apply).
    #[serde(default)]
    #[allow(dead_code)]
    system: Option<Value>,
    /// Status subresource (read-only, ignored during apply).
    #[serde(default)]
    #[allow(dead_code)]
    status: Option<Value>,
}

/// Metadata fields used in an apply manifest.
#[derive(Debug, Deserialize)]
struct ApplyMetadata {
    /// Object name.
    name: String,
    /// Target namespace (optional — uses CLI flag or default).
    namespace: Option<String>,
    /// Labels to set on the object.
    #[serde(default)]
    labels: HashMap<String, String>,
    /// Annotations to set on the object.
    #[serde(default)]
    annotations: HashMap<String, String>,
    /// Finalizers on the object.
    #[serde(default)]
    finalizers: Vec<String>,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Err(err) = run(cli).await {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

/// Top-level dispatcher.
async fn run(cli: Cli) -> Result<(), CliError> {
    let config = Config::load()?;
    let client = KapiClient::new(&config.server).map_err(CliError::ClientError)?;
    let resolver = SchemaResolver::new(&client).await?;

    match cli.command {
        Commands::Edit { kind, name } => {
            cmd_edit(&client, &resolver, &kind, &name, cli.namespace.as_deref(), cli.all_namespaces)
                .await
        }
        Commands::Get { kind, name, label_selector, limit } => {
            cmd_get(
                &client,
                &resolver,
                &kind,
                name.as_deref(),
                label_selector.as_deref(),
                limit,
                &cli.output,
                cli.namespace.as_deref(),
                cli.all_namespaces,
            )
            .await
        }
        Commands::Apply { file } => {
            cmd_apply(&client, &resolver, &file, &cli.output, cli.namespace.as_deref()).await
        }
        Commands::Delete { kind, name } => {
            cmd_delete(
                &client,
                &resolver,
                &kind,
                &name,
                &cli.output,
                cli.namespace.as_deref(),
                cli.all_namespaces,
            )
            .await
        }
        Commands::Watch { kind, label_selector } => {
            cmd_watch(
                &client,
                &resolver,
                &kind,
                label_selector.as_deref(),
                cli.namespace.as_deref(),
                cli.all_namespaces,
            )
            .await
        }
        Commands::Status(cmd) => match cmd {
            StatusCommands::Get { kind, name } => {
                cmd_status_get(
                    &client,
                    &resolver,
                    &kind,
                    &name,
                    &cli.output,
                    cli.namespace.as_deref(),
                    cli.all_namespaces,
                )
                .await
            }
            StatusCommands::Apply { kind, name, file } => {
                cmd_status_apply(
                    &client,
                    &resolver,
                    &kind,
                    &name,
                    &file,
                    &cli.output,
                    cli.namespace.as_deref(),
                    cli.all_namespaces,
                )
                .await
            }
        },
        Commands::Completions { shell } => {
            cmd_completions(shell);
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Namespace resolution helper
// ---------------------------------------------------------------------------

/// Resolves the effective namespace for a command.
///
/// - **Namespaced** scope: use the provided flag, or default to `"default"`.
/// - **Cluster** scope: always `None` (prints a warning if a namespace was given).
fn resolve_namespace(
    scope: &str,
    namespace_flag: Option<&str>,
    all_namespaces: bool,
) -> Option<String> {
    match scope {
        "Namespaced" => {
            if all_namespaces {
                return None;
            }
            if let Some(ns) = namespace_flag
                && !ns.is_empty()
            {
                return Some(ns.to_string());
            }
            Some("default".to_string())
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Output dispatch helpers
// ---------------------------------------------------------------------------

/// Prints a single `StoredObject` according to the output format.
fn print_object(obj: &StoredObject, fmt: &OutputFormat, scope: &str) -> Result<(), CliError> {
    match fmt {
        OutputFormat::Table => {
            // Print header + single row.
            if scope == "Namespaced" {
                println!("{:<30} {:<20} {:<10}", "NAME", "NAMESPACE", "AGE");
            } else {
                println!("{:<30} {:<10}", "NAME", "AGE");
            }
            println!("{}", format_table(obj, scope));
        }
        OutputFormat::Json => {
            println!("{}", format_json(obj)?);
        }
        OutputFormat::Yaml => {
            print!("{}", format_yaml(obj)?);
        }
    }
    Ok(())
}

/// Prints a list of `StoredObject`s according to the output format.
fn print_object_list(
    items: &[StoredObject],
    fmt: &OutputFormat,
    scope: &str,
) -> Result<(), CliError> {
    match fmt {
        OutputFormat::Table => {
            print!("{}", format_table_list(items, scope));
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(items)?);
        }
        OutputFormat::Yaml => {
            print!("{}", serde_yaml::to_string(items)?);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Command:  get
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
async fn cmd_get(
    client: &KapiClient,
    resolver: &SchemaResolver,
    kind: &str,
    name: Option<&str>,
    label_selector: Option<&str>,
    limit: Option<usize>,
    output: &OutputFormat,
    namespace_flag: Option<&str>,
    all_namespaces: bool,
) -> Result<(), CliError> {
    let (key, scope) = resolver.resolve_kind(kind)?;

    // Cluster-scoped kind with -n flag: warn and ignore.
    if scope != "Namespaced" && namespace_flag.is_some() {
        eprintln!("Warning: kind '{kind}' is cluster-scoped, ignoring --namespace flag");
    }

    let namespace = resolve_namespace(&scope, namespace_flag, all_namespaces);

    if let Some(name) = name {
        let obj = client
            .get(&key, namespace.as_deref(), name)
            .await
            .map_err(|e| CliError::from_not_found(e, kind, name, namespace.as_deref()))?;
        print_object(&obj, output, &scope)?;
    } else {
        // Parse label selector if provided.
        let ls = if let Some(raw) = label_selector {
            if raw.is_empty() {
                None
            } else {
                match LabelSelector::parse(raw) {
                    Ok(WatchFilter::LabelSelector(sel)) => Some(sel),
                    Ok(_) => None,
                    Err(e) => {
                        return Err(CliError::FormatError(format!("invalid label selector: {e}")));
                    }
                }
            }
        } else {
            None
        };

        // Auto-paginate: follow continue_token until exhausted.
        let all_items = paginate_list(client, &key, namespace.as_deref(), ls, limit).await?;
        print_object_list(&all_items, output, &scope)?;
    }

    Ok(())
}

/// Lists objects with optional label selector, auto-paginating if no limit is set.
async fn paginate_list(
    client: &KapiClient,
    key: &ResourceKey,
    namespace: Option<&str>,
    label_selector: Option<LabelSelector>,
    limit: Option<usize>,
) -> Result<Vec<StoredObject>, CliError> {
    let mut all_items = Vec::new();
    let mut continue_token: Option<ContinueToken> = None;

    loop {
        let opts = ListOptions {
            limit: None, // Let server choose page size.
            continue_token: continue_token.take(),
            field_selector: None,
            label_selector: label_selector.clone(),
        };

        let ListResponse { items, continue_token: ct } = client.list(key, namespace, &opts).await?;

        // Determine how many items we can still accept.
        let remaining = limit.map(|l| l.saturating_sub(all_items.len()));
        match remaining {
            Some(0) => break, // Already at limit.
            Some(r) if r < items.len() => {
                // Take only up to the remaining capacity.
                all_items.extend(items.into_iter().take(r));
                break;
            }
            _ => {
                all_items.extend(items);
            }
        }

        continue_token = ct;

        // If server returned no continuation token, we're done.
        if continue_token.is_none() {
            break;
        }
    }

    Ok(all_items)
}

// ---------------------------------------------------------------------------
// Command:  apply
// ---------------------------------------------------------------------------

async fn cmd_apply(
    client: &KapiClient,
    resolver: &SchemaResolver,
    file_path: &str,
    output: &OutputFormat,
    namespace_flag: Option<&str>,
) -> Result<(), CliError> {
    // 1. Read and parse the manifest (from file or stdin).
    let content = if file_path == "-" {
        use std::io::Read;
        let mut buffer = String::new();
        std::io::stdin().read_to_string(&mut buffer)?;
        buffer
    } else {
        std::fs::read_to_string(file_path)?
    };

    let manifest: ApplyManifest = parse_manifest(&content, file_path)?;

    // 2. Extract group/version from apiVersion.
    let (group, _version) = manifest.api_version.split_once('/').ok_or_else(|| {
        CliError::FormatError(format!(
            "invalid apiVersion '{}': expected 'group/version' format",
            manifest.api_version
        ))
    })?;

    // 3. Resolve kind (uses group for disambiguation).
    let full_kind = format!("{}/{}", group, manifest.kind);
    let (key, scope) = resolver.resolve_kind(&full_kind)?;

    // 4. Determine namespace.
    let ns_from_file = manifest.metadata.namespace.as_deref();
    let namespace = resolve_apply_namespace(&scope, ns_from_file, namespace_flag)?;

    // 5. Try to GET the existing object.
    let existing = client.get(&key, namespace.as_deref(), &manifest.metadata.name).await;

    match existing {
        Ok(existing_obj) => {
            // --- Update ---
            let mut merged = existing_obj.clone();

            // Replace spec entirely.
            merged.spec = manifest.spec;

            // Merge labels additively (file values override).
            for (k, v) in &manifest.metadata.labels {
                merged.metadata.labels.insert(k.clone(), v.clone());
            }

            // Merge annotations additively (file values override).
            for (k, v) in &manifest.metadata.annotations {
                merged.metadata.annotations.insert(k.clone(), v.clone());
            }

            let updated = client.update(namespace.as_deref(), &merged).await?;
            print_object(&updated, output, &scope)?;
        }
        Err(ClientError::ApiError { status: 404, .. }) => {
            // --- Create ---
            let meta = ObjectMeta {
                name: manifest.metadata.name.clone(),
                namespace: namespace.clone(),
                labels: manifest.metadata.labels.clone(),
                annotations: manifest.metadata.annotations.clone(),
                finalizers: Vec::new(),
            };
            // Schema kind expects fields at top level (not wrapped in spec).
            let created = if key.kind == "Schema" {
                client.create_schema(&meta, &manifest.spec).await?
            } else {
                client.create(&key, namespace.as_deref(), &meta, &manifest.spec).await?
            };
            print_object(&created, output, &scope)?;
        }
        Err(other) => return Err(CliError::from(other)),
    }

    Ok(())
}

/// Resolves namespace for the apply command.
///
/// Priority: file metadata.namespace > --namespace flag > "default"
fn resolve_apply_namespace(
    scope: &str,
    ns_from_file: Option<&str>,
    namespace_flag: Option<&str>,
) -> Result<Option<String>, CliError> {
    match scope {
        "Namespaced" => {
            let ns = ns_from_file.or(namespace_flag).filter(|s| !s.is_empty()).unwrap_or("default");
            Ok(Some(ns.to_string()))
        }
        _ => {
            if namespace_flag.is_some() {
                eprintln!("Warning: this kind is cluster-scoped, ignoring --namespace flag");
            }
            Ok(None)
        }
    }
}

/// Parses a manifest from file content, trying JSON first, then YAML.
fn parse_manifest(content: &str, path: &str) -> Result<ApplyManifest, CliError> {
    let ext = Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("");

    match ext {
        "json" => Ok(serde_json::from_str(content)?),
        "yaml" | "yml" => Ok(serde_yaml::from_str(content)?),
        _ => {
            // Unknown extension: try YAML first (it accepts JSON too),
            // fall back to JSON.
            serde_yaml::from_str(content)
                .or_else(|_| serde_json::from_str(content))
                .map_err(|e| CliError::FormatError(format!("failed to parse manifest: {e}")))
        }
    }
}

// ---------------------------------------------------------------------------
// Command:  delete
// ---------------------------------------------------------------------------

async fn cmd_delete(
    client: &KapiClient,
    resolver: &SchemaResolver,
    kind: &str,
    name: &str,
    _output: &OutputFormat,
    namespace_flag: Option<&str>,
    all_namespaces: bool,
) -> Result<(), CliError> {
    let (key, scope) = resolver.resolve_kind(kind)?;

    if scope != "Namespaced" && namespace_flag.is_some() {
        eprintln!("Warning: kind '{kind}' is cluster-scoped, ignoring --namespace flag");
    }

    let namespace = resolve_namespace(&scope, namespace_flag, all_namespaces);

    let deleted = client
        .delete(&key, namespace.as_deref(), name)
        .await
        .map_err(|e| CliError::from_not_found(e, kind, name, namespace.as_deref()))?;
    println!("{} '{}' deleted", kind, deleted.metadata.name);

    Ok(())
}

// ---------------------------------------------------------------------------
// Command:  edit
// ---------------------------------------------------------------------------

async fn cmd_edit(
    client: &KapiClient,
    resolver: &SchemaResolver,
    kind: &str,
    name: &str,
    namespace_flag: Option<&str>,
    all_namespaces: bool,
) -> Result<(), CliError> {
    // 1. Resolve kind.
    let (key, scope) = resolver.resolve_kind(kind)?;

    if scope != "Namespaced" && namespace_flag.is_some() {
        eprintln!("Warning: kind '{kind}' is cluster-scoped, ignoring --namespace flag");
    }

    // 2. Determine namespace for fetching.
    let fetch_namespace = resolve_namespace(&scope, namespace_flag, all_namespaces);

    // 3. Fetch current object.
    let obj = client
        .get(&key, fetch_namespace.as_deref(), name)
        .await
        .map_err(|e| CliError::from_not_found(e, kind, name, fetch_namespace.as_deref()))?;

    // 4. Serialize to YAML for editing.
    let api_version = format!("{}/{}", obj.key.group, obj.key.version);
    let doc = serde_json::json!({
        "apiVersion": api_version,
        "kind": obj.key.kind,
        "metadata": {
            "name": obj.metadata.name,
            "namespace": obj.metadata.namespace,
            "labels": obj.metadata.labels,
            "annotations": obj.metadata.annotations,
            "finalizers": obj.metadata.finalizers,
        },
        "spec": obj.spec,
        "status": obj.status,
        "system": {
            "resourceVersion": obj.system.resource_version,
            "generation": obj.system.generation,
            "createdAt": obj.system.created_at,
            "updatedAt": obj.system.updated_at,
            "deletionTimestamp": obj.system.deletion_timestamp,
        },
    });
    let yaml = serde_yaml::to_string(&doc)?;

    // 5. Write to a temporary file.
    let mut tmp = tempfile::NamedTempFile::new()?;
    let tmp_path = tmp.path().to_path_buf();
    use std::io::Write;
    tmp.write_all(yaml.as_bytes())?;
    tmp.flush()?;
    // Keep the handle alive so the file isn't deleted before the editor reads it.

    // 6. Determine the editor to use.
    let editor =
        std::env::var("EDITOR").or_else(|_| std::env::var("VISUAL")).unwrap_or_else(|_| {
            if cfg!(target_os = "windows") { "notepad".to_string() } else { "vi".to_string() }
        });

    // 7. Open the editor and wait for it to close.
    let status = std::process::Command::new(&editor)
        .arg(&tmp_path)
        .status()
        .map_err(|e| CliError::FormatError(format!("failed to launch editor '{editor}': {e}")))?;

    if !status.success() {
        // Editor exited with an error — clean up temp file.
        let _ = std::fs::remove_file(&tmp_path);
        return Err(CliError::FormatError(format!(
            "editor '{editor}' exited with status {:?}",
            status.code()
        )));
    }

    // 8. Read the edited content.
    let content = std::fs::read_to_string(&tmp_path)?;

    // 9. Parse as ApplyManifest.
    let manifest: ApplyManifest = serde_yaml::from_str(&content).map_err(|e| {
        // Keep temp file for recovery.
        CliError::FormatError(format!(
            "invalid edited content: {e}\nFile kept at: {}",
            tmp_path.display()
        ))
    })?;

    // 10. Determine namespace for applying (file value takes priority).
    let ns_from_file = manifest.metadata.namespace.as_deref();
    let apply_namespace = resolve_apply_namespace(&scope, ns_from_file, namespace_flag)?;

    // 11. GET the current object again (to get the latest resourceVersion).
    let existing = client
        .get(&key, apply_namespace.as_deref(), &manifest.metadata.name)
        .await
        .map_err(|e| {
            CliError::from_not_found(e, kind, &manifest.metadata.name, apply_namespace.as_deref())
        })?;

    // 12. Merge edited fields while preserving system metadata.
    let mut merged = existing.clone();
    merged.spec = manifest.spec;
    for (k, v) in &manifest.metadata.labels {
        merged.metadata.labels.insert(k.clone(), v.clone());
    }
    for (k, v) in &manifest.metadata.annotations {
        merged.metadata.annotations.insert(k.clone(), v.clone());
    }
    // Replace finalizers entirely (allows removal).
    merged.metadata.finalizers = manifest.metadata.finalizers;

    // 13. PUT the updated object.
    let updated = client.update(apply_namespace.as_deref(), &merged).await?;
    println!("{} '{}' edited", kind, updated.metadata.name);

    // 14. Clean up temp file.
    let _ = std::fs::remove_file(&tmp_path);

    Ok(())
}

// ---------------------------------------------------------------------------
// Command:  watch
// ---------------------------------------------------------------------------

async fn cmd_watch(
    client: &KapiClient,
    resolver: &SchemaResolver,
    kind: &str,
    label_selector: Option<&str>,
    namespace_flag: Option<&str>,
    all_namespaces: bool,
) -> Result<(), CliError> {
    let (key, scope) = resolver.resolve_kind(kind)?;

    if scope != "Namespaced" && namespace_flag.is_some() {
        eprintln!("Warning: kind '{kind}' is cluster-scoped, ignoring --namespace flag");
    }

    let namespace = resolve_namespace(&scope, namespace_flag, all_namespaces);

    // Build watch filter from label selector.
    let filter = match label_selector {
        Some(raw) if !raw.is_empty() => LabelSelector::parse(raw)?,
        _ => WatchFilter::All,
    };

    let mut stream = client.watch(&key, namespace.as_deref(), &filter).await?;

    // Print header once.
    if scope == "Namespaced" {
        println!("{:<20} {:<30} {:<20} {:<10}", "EVENT_TYPE", "NAME", "NAMESPACE", "AGE");
    } else {
        println!("{:<20} {:<30} {:<10}", "EVENT_TYPE", "NAME", "AGE");
    }

    while let Some(event_result) = stream.next().await {
        match event_result {
            Ok(event) => {
                println!("{}", format_watch_event(&event, &scope));
            }
            Err(err) => {
                eprintln!("Watch error: {err}");
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Command:  status get
// ---------------------------------------------------------------------------

async fn cmd_status_get(
    client: &KapiClient,
    resolver: &SchemaResolver,
    kind: &str,
    name: &str,
    output: &OutputFormat,
    namespace_flag: Option<&str>,
    all_namespaces: bool,
) -> Result<(), CliError> {
    let (key, scope) = resolver.resolve_kind(kind)?;

    if scope != "Namespaced" && namespace_flag.is_some() {
        eprintln!("Warning: kind '{kind}' is cluster-scoped, ignoring --namespace flag");
    }

    let namespace = resolve_namespace(&scope, namespace_flag, all_namespaces);

    let status = client
        .get_status(&key, namespace.as_deref(), name)
        .await
        .map_err(|e| CliError::from_not_found(e, kind, name, namespace.as_deref()))?;

    match status {
        Some(val) => match output {
            OutputFormat::Table => {
                println!("Status of {} '{}':", kind, name);
                println!("{}", serde_json::to_string_pretty(&val)?);
            }
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&val)?);
            }
            OutputFormat::Yaml => {
                print!("{}", serde_yaml::to_string(&val)?);
            }
        },
        None => {
            println!("No status set for {kind} '{name}'");
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Command:  status apply
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
async fn cmd_status_apply(
    client: &KapiClient,
    resolver: &SchemaResolver,
    kind: &str,
    name: &str,
    file_path: &str,
    output: &OutputFormat,
    namespace_flag: Option<&str>,
    all_namespaces: bool,
) -> Result<(), CliError> {
    let (key, scope) = resolver.resolve_kind(kind)?;

    if scope != "Namespaced" && namespace_flag.is_some() {
        eprintln!("Warning: kind '{kind}' is cluster-scoped, ignoring --namespace flag");
    }

    let namespace = resolve_namespace(&scope, namespace_flag, all_namespaces);

    // Read and parse the status value from file.
    let content = std::fs::read_to_string(file_path)?;
    let status_value: Value = parse_json_or_yaml_value(&content, file_path)?;

    let updated = client.update_status(&key, namespace.as_deref(), name, &status_value).await?;

    print_object(&updated, output, &scope)?;

    Ok(())
}

/// Parses a JSON or YAML file into a generic Value.
fn parse_json_or_yaml_value(content: &str, path: &str) -> Result<Value, CliError> {
    let ext = Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("");

    match ext {
        "json" => Ok(serde_json::from_str(content)?),
        "yaml" | "yml" => Ok(serde_yaml::from_str(content)?),
        _ => serde_yaml::from_str(content)
            .or_else(|_| serde_json::from_str(content))
            .map_err(|e| CliError::FormatError(format!("failed to parse file: {e}"))),
    }
}

// ---------------------------------------------------------------------------
// Command:  completions
// ---------------------------------------------------------------------------

fn cmd_completions(shell: clap_complete::Shell) {
    use clap::CommandFactory;

    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    clap_complete::generate(shell, &mut cmd, name, &mut std::io::stdout());
}
