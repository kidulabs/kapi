//! Controller generation logic for kapibuild.
//! Generates reconciler scaffolding for API resources.
//!
//! Supports two modes:
//! - **Auto-discovery** (no args): scans all registered APIs and generates
//!   controllers for any that are missing one.
//! - **Specific resource** (--group, --version, --kind): generates a controller
//!   for exactly one resource.

use std::path::Path;

use anyhow::{Context, Result};

use crate::generate::capitalize_first;
use crate::{Kapifile, find_project_root};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Execute the `kapibuild controller generate` command.
///
/// When `--group`, `--version`, and `--kind` are all provided, generates a
/// controller for that specific resource. When none are provided, discovers
/// all registered APIs and generates controllers for any that are missing one.
pub fn cmd_controller_generate(args: crate::ControllerGenerateArgs) -> Result<()> {
    let project_root = find_project_root()?;

    let has_group = args.group.is_some();
    let has_version = args.version.is_some();
    let has_kind = args.kind.is_some();

    // Validate: either all three flags or none.
    let specified = [has_group, has_version, has_kind];
    if specified.iter().any(|&s| s) && !specified.iter().all(|&s| s) {
        anyhow::bail!(
            "--group, --version, and --kind must all be provided together, or all omitted"
        );
    }

    if specified[0] {
        // Specific-resource mode.
        let group = args.group.as_deref().unwrap();
        let version = args.version.as_deref().unwrap();
        let kind = args.kind.as_deref().unwrap();
        generate_single_controller(&project_root, kind, group, version)?;
    } else {
        // Auto-discovery mode.
        let kapifile = read_kapifile(&project_root)?;
        let resources = discover_resources(&project_root, &kapifile)?;

        if resources.is_empty() {
            anyhow::bail!("no API resources found — run 'kapibuild api create' first");
        }

        let mut generated = 0u32;
        let mut skipped = 0u32;

        for res in &resources {
            let kind_lower = res.kind.to_lowercase();
            let controller_file = project_root
                .join("src")
                .join("controllers")
                .join(format!("{kind_lower}_controller.rs"));

            if controller_file.exists() {
                skipped += 1;
                continue;
            }

            generate_single_controller(&project_root, &res.kind, &res.group, &res.version)?;
            generated += 1;
        }
        if generated == 0 {
            println!("All {skipped} controller(s) already exist. Nothing to generate.");
        } else {
            println!("Generated {generated} controller(s), {skipped} already existed.");
        }
    }

    Ok(())
}

/// Generate a controller for a single resource.
fn generate_single_controller(
    project_root: &Path,
    kind: &str,
    group: &str,
    version: &str,
) -> Result<()> {
    let kind_lower = kind.to_lowercase();

    // Validate: src/api/<group>/<version>/<kind_lower>.rs must exist.
    let api_file_path = project_root
        .join("src")
        .join("api")
        .join(group)
        .join(version)
        .join(format!("{kind_lower}.rs"));

    if !api_file_path.exists() {
        anyhow::bail!(
            "API resource '{kind}' not found. Run 'kapibuild api create --group {group} --version {version} --kind {kind}' first."
        );
    }

    // Validate: kind must be registered in Kapifile.
    let kapifile = read_kapifile(project_root)?;
    let resource = kapifile
        .resources
        .iter()
        .find(|r| r.kind == kind)
        .with_context(|| {
            format!(
                "API resource '{kind}' not found. Run 'kapibuild api create --group {group} --version {version} --kind {kind}' first."
            )
        })?;

    let has_status = resource.has_status;

    // Skip if controller already exists (idempotent in auto-discovery mode).
    let controllers_dir = project_root.join("src").join("controllers");
    let controller_file = controllers_dir.join(format!("{kind_lower}_controller.rs"));

    if controller_file.exists() {
        println!("Skipping {kind}: controller already exists at '{}'", controller_file.display());
        return Ok(());
    }

    // Generate and write the controller file.
    let content = generate_controller_content(kind, group, version, has_status);
    std::fs::write(&controller_file, &content)
        .with_context(|| format!("failed to write '{}'", controller_file.display()))?;

    println!("Created controller: {}", controller_file.display());

    // Update src/controllers/mod.rs.
    update_controllers_mod(project_root, kind, &kind_lower)?;

    // Update src/main.rs.
    update_main_rs(project_root, kind, group, version, &kind_lower)?;

    println!("Controller '{kind}Reconciler' generated and wired successfully.");

    Ok(())
}

// ---------------------------------------------------------------------------
// Auto-discovery
// ---------------------------------------------------------------------------

/// A discovered API resource from the filesystem.
struct DiscoveredResource {
    group: String,
    version: String,
    kind: String,
}

/// Scan `api/` directory and discover all resources.
fn discover_resources(
    project_root: &Path,
    _kapifile: &Kapifile,
) -> Result<Vec<DiscoveredResource>> {
    let api_dir = project_root.join("src").join("api");
    if !api_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut resources = Vec::new();

    // Walk api/<group>/<version>/<kind>.rs
    for group_entry in std::fs::read_dir(&api_dir).context("failed to read api directory")? {
        let group_entry = group_entry?;
        if !group_entry.path().is_dir() {
            continue;
        }
        let group = group_entry.file_name().to_string_lossy().to_string();

        for version_entry in std::fs::read_dir(group_entry.path())? {
            let version_entry = version_entry?;
            if !version_entry.path().is_dir() {
                continue;
            }
            let version = version_entry.file_name().to_string_lossy().to_string();

            for kind_entry in std::fs::read_dir(version_entry.path())? {
                let kind_entry = kind_entry?;
                let path = kind_entry.path();
                if path.is_file()
                    && path.extension().is_some_and(|e| e == "rs")
                    && path.file_stem().is_some_and(|s| s != "mod")
                {
                    let stem =
                        path.file_stem().and_then(|s| s.to_str()).context("invalid filename")?;
                    let kind = capitalize_first(stem);
                    resources.push(DiscoveredResource {
                        group: group.clone(),
                        version: version.clone(),
                        kind,
                    });
                }
            }
        }
    }

    Ok(resources)
}

// ---------------------------------------------------------------------------
// Controller content generation
// ---------------------------------------------------------------------------

/// Generate the Rust source for a controller reconciler file.
pub(crate) fn generate_controller_content(
    kind: &str,
    group: &str,
    version: &str,
    has_status: bool,
) -> String {
    let group_mod = group.replace('.', "_");
    let kind_lower = kind.to_lowercase();
    let finalizer_name = format!("controller.kapi.io/{kind_lower}-cleanup");

    let mut out = String::new();

    // File header.
    out.push_str(&format!(
        "//! Generated controller for {kind}.\n\
         //! DO NOT EDIT - generated by `kapibuild controller generate`.\n\n"
    ));

    // Imports.
    out.push_str(
        "use async_trait::async_trait;\n\
         use kapi_client::typed::TypedClient;\n\
         use kapi_controller::finalizer;\n\
         use kapi_controller::reconciler::{ReconcileContext, ReconcileResult, Reconciler};\n\n",
    );
    out.push_str(&format!("use crate::types::{group_mod}::{version}::{kind_lower}::{kind};\n\n"));

    // Finalizer constant.
    out.push_str(&format!(
        "/// The finalizer name for {kind} cleanup.\n\
         const FINALIZER_NAME: &str = \"{finalizer_name}\";\n\n"
    ));

    // Struct definition.
    out.push_str(&format!(
        "/// {kind}Reconciler reconciles {kind} resources.\n\
         pub struct {kind}Reconciler;\n\n"
    ));

    // Reconciler impl.
    out.push_str(&format!(
        "#[async_trait]\n\
         impl Reconciler for {kind}Reconciler {{\n\
         \x20   async fn reconcile(\n\
         \x20       &self,\n\
         \x20       ctx: ReconcileContext,\n\
         \x20   ) -> Result<ReconcileResult, Box<dyn std::error::Error + Send + Sync>> {{\n\
         \x20       let typed_client = TypedClient::<{kind}>::new(ctx.client.clone());\n\n\
         \x20       // Fetch the object using the typed client.\n\
         \x20       let _{kind_lower} = typed_client\n\
         \x20           .get(ctx.request.namespace.as_deref(), &ctx.request.name)\n\
         \x20           .await?;\n\n\
         \x20       // Handle deletion with finalizer pattern.\n\
         \x20       let obj = ctx.client\n\
         \x20           .get(&ctx.request.key, ctx.request.namespace.as_deref(), &ctx.request.name)\n\
         \x20           .await?;\n\n\
         \x20       if finalizer::is_deleting(&obj) {{\n\
         \x20           // TODO: Add cleanup logic here.\n\n\
         \x20           // Remove finalizer to allow deletion.\n\
         \x20           finalizer::remove_finalizer(&ctx.client, &obj, FINALIZER_NAME).await?;\n\
         \x20           return Ok(ReconcileResult::default());\n\
         \x20       }}\n\n\
         \x20       // Ensure finalizer is present.\n\
         \x20       finalizer::ensure_finalizer(&ctx.client, &obj, FINALIZER_NAME).await?;\n\n\
         \x20       // TODO: Add your reconciliation logic here.\n\
         \x20       // Use `{kind_lower}.spec()` to read the desired state.\n\n"
    ));

    if has_status {
        out.push_str(
            "\x20       // Update status to reflect observed state.\n\
             \x20       let status = serde_json::json!({\n\
             \x20           \"ready\": true,\n\
             \x20           \"message\": \"Reconciled successfully\",\n\
             \x20       });\n\
             \x20       typed_client\n\
             \x20           .inner()\n\
             \x20           .update_status(\n\
             \x20               &ctx.request.key,\n\
             \x20               ctx.request.namespace.as_deref(),\n\
             \x20               &ctx.request.name,\n\
             \x20               &status,\n\
             \x20           )\n\
             \x20           .await?;\n\n",
        );
    } else {
        out.push_str(
            "\x20       // Status updates omitted — resource has no status subresource.\n\n",
        );
    }

    out.push_str(
        "\x20       Ok(ReconcileResult::default())\n\
         \x20   }\n\
         }\n",
    );

    out
}

// ---------------------------------------------------------------------------
// Kapifile reading
// ---------------------------------------------------------------------------

/// Parse the Kapifile at the project root.
fn read_kapifile(project_root: &Path) -> Result<Kapifile> {
    let path = project_root.join("Kapifile");
    let content = std::fs::read_to_string(&path).context("failed to read Kapifile")?;
    let kapifile: Kapifile = serde_yaml::from_str(&content).context("failed to parse Kapifile")?;
    Ok(kapifile)
}

// ---------------------------------------------------------------------------
// Module wiring helpers
// ---------------------------------------------------------------------------

/// Add `pub mod` and `pub use` for the new controller in `src/controllers/mod.rs`.
fn update_controllers_mod(project_root: &Path, _kind: &str, kind_lower: &str) -> Result<()> {
    let mod_path = project_root.join("src").join("controllers").join("mod.rs");

    let content = std::fs::read_to_string(&mod_path)
        .with_context(|| format!("failed to read '{}'", mod_path.display()))?;

    let mod_line = format!("pub mod {kind_lower}_controller;");

    // Only append if not already present.
    if content.contains(&mod_line) {
        return Ok(());
    }

    let mut new_content = content;
    if !new_content.ends_with('\n') {
        new_content.push('\n');
    }
    new_content.push_str(&mod_line);
    new_content.push('\n');

    std::fs::write(&mod_path, &new_content)
        .with_context(|| format!("failed to write '{}'", mod_path.display()))?;

    println!("Updated: {}", mod_path.display());
    Ok(())
}

/// Add the controller wiring (imports + registration) to `src/main.rs`.
fn update_main_rs(
    project_root: &Path,
    kind: &str,
    group: &str,
    version: &str,
    kind_lower: &str,
) -> Result<()> {
    let main_path = project_root.join("src").join("main.rs");
    let content = std::fs::read_to_string(&main_path)
        .with_context(|| format!("failed to read '{}'", main_path.display()))?;

    let group_mod = group.replace('.', "_");
    let type_import = format!("use crate::types::{group_mod}::{version}::{kind_lower}::{kind};");
    let controller_import =
        format!("use crate::controllers::{kind_lower}_controller::{kind}Reconciler;");
    let registration = format!(
        "    manager.controller_for({kind}::key()).reconcile_with({kind}Reconciler).register();"
    );

    let mut new_content = content.clone();

    // --- Add imports (before #[tokio::main]) ---
    let mut imports_to_add = String::new();
    if !content.contains(&type_import) {
        imports_to_add.push_str(&format!("{type_import}\n"));
    }
    if !content.contains(&controller_import) {
        imports_to_add.push_str(&format!("{controller_import}\n"));
    }

    if !imports_to_add.is_empty() {
        let marker = "#[tokio::main]";
        if let Some(pos) = new_content.find(marker) {
            new_content.insert_str(pos, &imports_to_add);
        }
    }

    // --- Add controller registration ---
    if !new_content.contains(&registration) {
        let todo_block = "    // TODO: Register controllers here\n    // manager.controller_for(key).reconciler(my_reconciler).register();";

        if let Some(pos) = new_content.find(todo_block) {
            // Replace the TODO block with the registration line.
            let before = &new_content[..pos];
            let after = &new_content[pos + todo_block.len()..];
            // Skip any whitespace-only line that follows the TODO block.
            let after = after
                .strip_prefix('\n')
                .and_then(|rest| {
                    let trimmed = rest.trim_start_matches([' ', '\t']);
                    trimmed.strip_prefix('\n').or(Some(rest))
                })
                .unwrap_or(after);
            new_content = format!("{before}{registration}\n\n{after}");
        } else {
            // No TODO block; insert before the line containing `manager.start()`.
            if let Some(pos) = new_content.find("manager.start()") {
                let line_start = new_content[..pos].rfind('\n').map_or(0, |p| p + 1);
                new_content.insert_str(line_start, &format!("{registration}\n"));
            }
        }
    }

    std::fs::write(&main_path, &new_content)
        .with_context(|| format!("failed to write '{}'", main_path.display()))?;

    println!("Updated: {}", main_path.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_controller_content_with_status() {
        let code = generate_controller_content("Widget", "example.io", "v1", true);
        assert!(code.contains("pub struct WidgetReconciler"));
        assert!(code.contains("impl Reconciler for WidgetReconciler"));
        assert!(code.contains("FINALIZER_NAME"));
        assert!(code.contains("controller.kapi.io/widget-cleanup"));
        assert!(code.contains("TypedClient::<Widget>::new"));
        assert!(code.contains("finalizer::is_deleting"));
        assert!(code.contains("finalizer::ensure_finalizer"));
        assert!(code.contains("finalizer::remove_finalizer"));
        assert!(code.contains("update_status"));
        assert!(code.contains("crate::types::example_io::v1::widget::Widget"));
    }

    #[test]
    fn test_generate_controller_content_without_status() {
        let code = generate_controller_content("Gadget", "test.io", "v1", false);
        assert!(code.contains("pub struct GadgetReconciler"));
        assert!(!code.contains("update_status"));
        assert!(code.contains("Status updates omitted"));
    }

    #[test]
    fn test_generate_controller_content_has_imports() {
        let code = generate_controller_content("Widget", "example.io", "v1", true);
        assert!(code.contains("use async_trait::async_trait;"));
        assert!(code.contains("use kapi_client::typed::TypedClient;"));
        assert!(code.contains("use kapi_controller::finalizer;"));
        assert!(code.contains(
            "use kapi_controller::reconciler::{ReconcileContext, ReconcileResult, Reconciler};"
        ));
    }

    #[test]
    fn test_generate_controller_content_without_status_comment() {
        let code = generate_controller_content("Gadget", "test.io", "v1", false);
        assert!(code.contains("// Status updates omitted — resource has no status subresource."));
    }
}
