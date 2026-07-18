//! Schema generation via a temporary helper binary.
//!
//! Scans `api/` directory for resource definition files, generates wrapper
//! structs in `types/`, and runs a temporary Cargo project to produce JSON
//! schema files in `schemas/`.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use regex::Regex;

use crate::find_project_root;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Execute the `kapibuild api generate` command.
pub fn cmd_api_generate() -> Result<()> {
    let project_root = find_project_root()?;
    let ws_root = workspace_root();

    let kapifile = read_kapifile(&project_root)?;
    let resources = scan_api_dir(&project_root, &kapifile)?;
    if resources.is_empty() {
        anyhow::bail!("no API resources found in api/ directory");
    }

    // Generate types/ wrapper files.
    for res in &resources {
        let type_path = generate_type_file(&project_root, res)?;
        println!("Generated wrapper: {}", type_path.display());
    }

    // Create and run helper project for schema generation.
    let tmp = create_helper_project(&resources, &project_root, &ws_root)
        .context("failed to create helper project")?;

    let manifest_path = tmp.path().join("Cargo.toml");

    let output = Command::new("cargo")
        .args(["run", "--manifest-path", &manifest_path.to_string_lossy(), "-q"])
        .output()
        .context("failed to execute cargo run for schema helper")?;

    // Print stdout / stderr from the helper regardless.
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.is_empty() {
        print!("{stdout}");
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() {
        eprint!("{stderr}");
    }

    if !output.status.success() {
        anyhow::bail!("schema helper failed (exit code: {:?})", output.status.code());
    }

    println!("Schema generation complete.");
    Ok(())
}

// ---------------------------------------------------------------------------
// Kapifile reading
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
struct Kapifile {
    #[allow(dead_code)]
    domain: String,
    #[allow(dead_code)]
    version: String,
    #[serde(default)]
    resources: Vec<KapifileResource>,
}

#[derive(Debug, serde::Deserialize)]
struct KapifileResource {
    kind: String,
    #[allow(dead_code)]
    version: String,
    scope: String,
    #[serde(default)]
    has_status: bool,
}

fn read_kapifile(project_root: &Path) -> Result<Kapifile> {
    let path = project_root.join("Kapifile");
    let content = std::fs::read_to_string(&path).context("failed to read Kapifile")?;
    let kapifile: Kapifile = serde_yaml::from_str(&content).context("failed to parse Kapifile")?;
    Ok(kapifile)
}

// ---------------------------------------------------------------------------
// Workspace root detection
// ---------------------------------------------------------------------------

/// Locate the kapi workspace root by walking up from kapibuild's own
/// `Cargo.toml` until we find a manifest whose `[workspace]` members
/// include `"kapi-core"`.
fn workspace_root() -> PathBuf {
    // At compile-time CARGO_MANIFEST_DIR is set to kapibuild's directory.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // The workspace root is the parent of kapibuild/.
    manifest_dir.parent().expect("workspace root (parent of kapibuild)").to_path_buf()
}

// ---------------------------------------------------------------------------
// Resource scanning & parsing
// ---------------------------------------------------------------------------

/// Parsed metadata for a single API resource file.
#[derive(Debug, Clone)]
struct ResourceInfo {
    group: String,
    version: String,
    kind: String,
    scope: String,
    /// Name of the spec struct (e.g. `"WidgetSpec"`).
    spec_struct_name: String,
    /// Name of the status struct (e.g. `"WidgetStatus"`), if present.
    status_struct_name: Option<String>,
    /// Raw file content.
    content: String,
}

/// Walk `api/` recursively and collect all resource files, matching them
/// against entries in the Kapifile.
fn scan_api_dir(project_root: &Path, kapifile: &Kapifile) -> Result<Vec<ResourceInfo>> {
    let api_dir = project_root.join("api");
    if !api_dir.is_dir() {
        anyhow::bail!("api/ directory not found at '{}'", api_dir.display());
    }

    // Build a map from kind → KapifileResource for quick lookup.
    let mut resource_map: std::collections::HashMap<&str, &KapifileResource> =
        std::collections::HashMap::new();
    for r in &kapifile.resources {
        resource_map.insert(r.kind.as_str(), r);
    }

    let mut files = Vec::new();
    collect_rs_files(&api_dir, &mut files)?;

    let mut resources = Vec::new();
    for path in &files {
        // Get relative path from api/ dir.
        let relative = path
            .strip_prefix(&api_dir)
            .with_context(|| format!("path '{}' is not under api/", path.display()))?;

        // Expected structure: <group>/<version>/<kind>.rs  (3 components)
        let components: Vec<_> = relative.components().collect();
        if components.len() != 3 {
            // Skip files that don't match the expected layout.
            continue;
        }

        let group = components[0]
            .as_os_str()
            .to_str()
            .context("non-UTF-8 group directory name")?
            .to_string();
        let version = components[1]
            .as_os_str()
            .to_str()
            .context("non-UTF-8 version directory name")?
            .to_string();
        let kind_filename =
            components[2].as_os_str().to_str().context("non-UTF-8 filename")?.to_string();

        // Extract kind from filename: "widget.rs" -> "Widget"
        let kind_stem = kind_filename.strip_suffix(".rs").context("file does not end with .rs")?;
        let kind = capitalize_first(kind_stem);

        // Look up Kapifile resource entry by kind.
        let kapifile_entry = resource_map.get(kind.as_str()).with_context(|| {
            format!("resource '{}' found in api/ but not registered in Kapifile", kind)
        })?;

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read '{}'", path.display()))?;

        let spec_struct_name = format!("{}Spec", kind);
        let status_struct_name =
            if kapifile_entry.has_status { Some(format!("{}Status", kind)) } else { None };

        // Verify that the spec struct exists in the file.
        if !content.contains(&format!("struct {}", spec_struct_name)) {
            anyhow::bail!(
                "expected spec struct '{}' not found in {}",
                spec_struct_name,
                path.display()
            );
        }

        // Verify that the status struct exists if has_status is true.
        if let Some(ref status_name) = status_struct_name {
            if !content.contains(&format!("struct {}", status_name)) {
                anyhow::bail!(
                    "expected status struct '{}' not found in {}",
                    status_name,
                    path.display()
                );
            }
        }

        resources.push(ResourceInfo {
            group,
            version,
            kind,
            scope: kapifile_entry.scope.clone(),
            spec_struct_name,
            status_struct_name,
            content,
        });
    }

    Ok(resources)
}

/// Recursively collect all `.rs` files under `dir`.
fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir).context("failed to read api directory")? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

/// Capitalize the first character of a string.
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().to_string() + chars.as_str(),
    }
}

// ---------------------------------------------------------------------------
// Type file generation
// ---------------------------------------------------------------------------

/// Generate the wrapper struct file at `types/<group>/<version>/<kind>.rs`
/// and return the path to the generated file.
fn generate_type_file(project_root: &Path, res: &ResourceInfo) -> Result<PathBuf> {
    let group_mod = res.group.replace('.', "_");
    let version_mod = &res.version;
    let kind_mod = res.kind.to_lowercase();

    let types_dir = project_root.join("types").join(&group_mod).join(version_mod);
    std::fs::create_dir_all(&types_dir)
        .with_context(|| format!("failed to create types directory '{}'", types_dir.display()))?;

    let file_path = types_dir.join(format!("{kind_mod}.rs"));

    let mut code = String::new();

    // File header.
    code.push_str(&format!(
        "//! Generated types for {}.\n\
         //! DO NOT EDIT - generated by `kapibuild api generate`.\n\n",
        res.kind
    ));

    // Imports.
    code.push_str(
        "use kapi_client::typed::TypedResource;\n\
         use kapi_core::ObjectMeta;\n\
         use kapi_core::ResourceKey;\n\
         use kapi_core::SystemMetadata;\n\
         use schemars::JsonSchema;\n\
         use serde::{Deserialize, Serialize};\n\n",
    );

    // Re-export spec from api module.
    code.push_str(&format!(
        "use crate::api::{group_mod}::{version_mod}::{kind_mod}::{};\n",
        res.spec_struct_name,
        group_mod = group_mod,
        version_mod = version_mod,
        kind_mod = kind_mod,
    ));

    if let Some(ref status_name) = res.status_struct_name {
        code.push_str(&format!(
            "use crate::api::{group_mod}::{version_mod}::{kind_mod}::{status_name};\n",
            status_name = status_name,
            group_mod = group_mod,
            version_mod = version_mod,
            kind_mod = kind_mod,
        ));
    }
    code.push('\n');

    // Wrapper struct doc comment.
    code.push_str(&format!(
        "/// {kind} is the wrapper struct for the {kind} resource.\n",
        kind = res.kind,
    ));

    // Wrapper struct definition.
    code.push_str(&format!(
        "#[derive(Debug, Clone, Serialize, Deserialize)]\n\
         #[serde(rename_all = \"camelCase\")]\n\
         pub struct {kind} {{\n\
         \x20   pub metadata: ObjectMeta,\n\
         \x20   pub system: SystemMetadata,\n\
         \x20   pub spec: {spec},\n",
        kind = res.kind,
        spec = res.spec_struct_name,
    ));

    if let Some(ref status_name) = res.status_struct_name {
        code.push_str(&format!(
            "\x20   #[serde(skip_serializing_if = \"Option::is_none\")]\n\
             \x20   pub status: Option<{status_name}>,\n",
        ));
    }

    code.push_str("}\n\n");

    // impl block with key() and schema_data().
    code.push_str(&format!(
        "impl {kind} {{\n\
         \x20   /// Returns the ResourceKey identifying this resource type.\n\
         \x20   pub fn key() -> ResourceKey {{\n\
         \x20       ResourceKey {{\n\
         \x20           group: {group:?}.to_string(),\n\
         \x20           version: {version:?}.to_string(),\n\
         \x20           kind: {kind:?}.to_string(),\n\
         \x20       }}\n\
         \x20   }}\n\n\
         \x20   /// Returns the schema registration payload for this resource.\n\
         \x20   pub fn schema_data() -> serde_json::Value {{\n\
         \x20       let spec_schema = schemars::schema_for!({spec});\n\
         \x20       let mut map = serde_json::Map::new();\n\
         \x20       map.insert(\"targetGroup\".into(), serde_json::Value::String({group:?}.to_string()));\n\
         \x20       map.insert(\"targetVersion\".into(), serde_json::Value::String({version:?}.to_string()));\n\
         \x20       map.insert(\"targetKind\".into(), serde_json::Value::String({kind:?}.to_string()));\n\
         \x20       map.insert(\"scope\".into(), serde_json::Value::String({scope:?}.to_string()));\n\
         \x20       map.insert(\"specSchema\".into(), serde_json::to_value(spec_schema).unwrap());\n",
        kind = res.kind,
        spec = res.spec_struct_name,
        group = res.group,
        version = res.version,
        scope = res.scope,
    ));

    if let Some(ref status_name) = res.status_struct_name {
        code.push_str(&format!(
            "\x20       let status_schema = schemars::schema_for!({status_name});\n\
             \x20       map.insert(\"statusSchema\".into(), serde_json::to_value(status_schema).unwrap());\n",
        ));
    }

    code.push_str(
        "\x20       serde_json::Value::Object(map)\n\
         \x20   }\n\
         }\n\n",
    );

    // impl TypedResource block.
    if let Some(ref status_name) = res.status_struct_name {
        // With status field.
        code.push_str(&format!(
            "impl TypedResource for {kind} {{\n\
             \x20   type Spec = {spec};\n\
             \x20   type Status = {status};\n\n\
             \x20   fn key() -> ResourceKey {{\n\
             \x20       ResourceKey {{\n\
             \x20           group: {group:?}.to_string(),\n\
             \x20           version: {version:?}.to_string(),\n\
             \x20           kind: {kind:?}.to_string(),\n\
             \x20       }}\n\
             \x20   }}\n\n\
             \x20   fn from_parts(\n\
             \x20       metadata: ObjectMeta,\n\
             \x20       system: SystemMetadata,\n\
             \x20       spec: Self::Spec,\n\
             \x20       status: Option<Self::Status>,\n\
             \x20   ) -> Self {{\n\
             \x20       Self {{ metadata, system, spec, status }}\n\
             \x20   }}\n\n\
             \x20   fn metadata(&self) -> &ObjectMeta {{\n\
             \x20       &self.metadata\n\
             \x20   }}\n\n\
             \x20   fn system(&self) -> &SystemMetadata {{\n\
             \x20       &self.system\n\
             \x20   }}\n\n\
             \x20   fn spec(&self) -> &Self::Spec {{\n\
             \x20       &self.spec\n\
             \x20   }}\n\n\
             \x20   fn status(&self) -> Option<&Self::Status> {{\n\
             \x20       self.status.as_ref()\n\
             \x20   }}\n\
             }}\n",
            kind = res.kind,
            spec = res.spec_struct_name,
            status = status_name,
            group = res.group,
            version = res.version,
        ));
    } else {
        // Without status field.
        code.push_str(&format!(
            "impl TypedResource for {kind} {{\n\
             \x20   type Spec = {spec};\n\
             \x20   type Status = serde_json::Value;\n\n\
             \x20   fn key() -> ResourceKey {{\n\
             \x20       ResourceKey {{\n\
             \x20           group: {group:?}.to_string(),\n\
             \x20           version: {version:?}.to_string(),\n\
             \x20           kind: {kind:?}.to_string(),\n\
             \x20       }}\n\
             \x20   }}\n\n\
             \x20   fn from_parts(\n\
             \x20       metadata: ObjectMeta,\n\
             \x20       system: SystemMetadata,\n\
             \x20       spec: Self::Spec,\n\
             \x20       _status: Option<Self::Status>,\n\
             \x20   ) -> Self {{\n\
             \x20       Self {{ metadata, system, spec }}\n\
             \x20   }}\n\n\
             \x20   fn metadata(&self) -> &ObjectMeta {{\n\
             \x20       &self.metadata\n\
             \x20   }}\n\n\
             \x20   fn system(&self) -> &SystemMetadata {{\n\
             \x20       &self.system\n\
             \x20   }}\n\n\
             \x20   fn spec(&self) -> &Self::Spec {{\n\
             \x20       &self.spec\n\
             \x20   }}\n\n\
             \x20   fn status(&self) -> Option<&Self::Status> {{\n\
             \x20       None\n\
             \x20   }}\n\
             }}\n",
            kind = res.kind,
            spec = res.spec_struct_name,
            group = res.group,
            version = res.version,
        ));
    }

    std::fs::write(&file_path, &code)
        .with_context(|| format!("failed to write '{}'", file_path.display()))?;

    Ok(file_path)
}

// ---------------------------------------------------------------------------
// Helper project generation
// ---------------------------------------------------------------------------

/// Create a temporary Cargo project that includes all discovered resources
/// and writes their schema JSON files when run.
fn create_helper_project(
    resources: &[ResourceInfo],
    project_root: &Path,
    workspace_root: &Path,
) -> Result<tempfile::TempDir> {
    let tmp = tempfile::tempdir().context("failed to create temp directory")?;
    let tmp_src = tmp.path().join("src");
    std::fs::create_dir(&tmp_src).context("failed to create src/ in helper project")?;

    // Write Cargo.toml
    write_helper_cargo_toml(tmp.path(), workspace_root)?;

    // Write main.rs
    write_helper_main_rs(tmp.path(), resources, project_root)?;

    Ok(tmp)
}

fn write_helper_cargo_toml(helper_root: &Path, workspace_root: &Path) -> Result<()> {
    let kapi_core_path = workspace_root.join("kapi-core");

    let content = format!(
        r#"[package]
name = "kapi-schema-helper"
version = "0.1.0"
edition = "2024"

[dependencies]
kapi-core = {{ path = "{}" }}
schemars = "1"
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
"#,
        kapi_core_path.display(),
    );

    std::fs::write(helper_root.join("Cargo.toml"), content)
        .context("failed to write helper Cargo.toml")
}

fn write_helper_main_rs(
    helper_root: &Path,
    resources: &[ResourceInfo],
    project_root: &Path,
) -> Result<()> {
    let schemas_dir = project_root.join("schemas");

    // Build the module-code for each resource.
    let mut module_code = String::new();
    let mut generate_calls = Vec::new();

    for (i, res) in resources.iter().enumerate() {
        let api_mod_name = format!("res_{i}_api");
        let mod_name = format!("res_{i}");

        // Prepare api module content (strip KapiResource-related imports).
        let api_content = prepare_resource_module(&res.content);

        // Generate wrapper struct and impl block (self-contained, no proc-macro).
        let wrapper_code = generate_wrapper_code(res);

        module_code.push_str(&format!(
            "mod {api_mod_name} {{\n{api_content}\n}}\n\
             #[allow(dead_code)]\n\
             mod {mod_name} {{\n\
             \x20   use super::{api_mod_name}::*;\n\
             \x20   use kapi_core::{{ObjectMeta, ResourceKey}};\n\
             \x20   use serde::{{Deserialize, Serialize}};\n\
             {wrapper_code}\n\
             }}\n",
        ));

        let filename = format!("{}_{}.json", res.group, res.kind);
        let schema_name = format!("{}.{}.{}", res.kind, res.group, res.version);
        generate_calls.push(format!(
            "let schema_data = {mod_name}::{kind}::schema_data();\n\
             let manifest = serde_json::json!({{\n\
             \x20   \"kind\": \"Schema\",\n\
             \x20   \"apiVersion\": \"kapi.io/v1\",\n\
             \x20   \"metadata\": {{ \"name\": {name:?} }},\n\
             \x20   \"spec\": schema_data,\n\
             }});\n\
             let filename = {f:?};\n\
             let path = schemas_dir.join(filename);\n\
             std::fs::write(&path, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();\n\
             eprintln!(\"Generated {{}}\", path.display());",
            f = filename,
            name = schema_name,
            kind = res.kind,
            mod_name = mod_name,
        ));
    }

    let main_rs = format!(
        r#"fn main() {{
    let schemas_dir = std::path::Path::new({schemas_dir:?});
    std::fs::create_dir_all(schemas_dir).unwrap();

    {generate_calls}
}}
"#,
        schemas_dir = schemas_dir,
        generate_calls = generate_calls.join("\n")
    );

    // Module code comes before main.
    let full_content = format!("{module_code}\n{main_rs}");

    std::fs::write(helper_root.join("src").join("main.rs"), &full_content)
        .context("failed to write helper main.rs")
}

/// Generate the wrapper struct code (without relying on proc-macro).
fn generate_wrapper_code(res: &ResourceInfo) -> String {
    let kind = &res.kind;
    let spec = &res.spec_struct_name;
    let has_status = res.status_struct_name.is_some();
    let status = res.status_struct_name.as_deref().unwrap_or("");

    let mut code = String::new();

    // Wrapper struct.
    code.push_str(&format!(
        "#[derive(Debug, Clone, Serialize, Deserialize)]\n\
         #[serde(rename_all = \"camelCase\")]\n\
         pub struct {kind} {{\n\
         \x20   pub metadata: ObjectMeta,\n\
         \x20   pub spec: {spec},\n",
    ));

    if has_status {
        code.push_str(&format!(
            "\x20   #[serde(skip_serializing_if = \"Option::is_none\")]\n\
             \x20   pub status: Option<{status}>,\n",
        ));
    }

    code.push_str("}\n\n");

    // impl block.
    code.push_str(&format!(
        "impl {kind} {{\n\
         \x20   pub fn key() -> ResourceKey {{\n\
         \x20       ResourceKey {{\n\
         \x20           group: {group:?}.to_string(),\n\
         \x20           version: {version:?}.to_string(),\n\
         \x20           kind: {kind:?}.to_string(),\n\
         \x20       }}\n\
         \x20   }}\n\n\
         \x20   pub fn schema_data() -> serde_json::Value {{\n\
         \x20       let spec_schema = schemars::schema_for!({spec});\n\
         \x20       let mut map = serde_json::Map::new();\n\
         \x20       map.insert(\"targetGroup\".into(), serde_json::Value::String({group:?}.to_string()));\n\
         \x20       map.insert(\"targetVersion\".into(), serde_json::Value::String({version:?}.to_string()));\n\
         \x20       map.insert(\"targetKind\".into(), serde_json::Value::String({kind:?}.to_string()));\n\
         \x20       map.insert(\"scope\".into(), serde_json::Value::String({scope:?}.to_string()));\n\
         \x20       map.insert(\"specSchema\".into(), serde_json::to_value(spec_schema).unwrap());\n",
        kind = kind,
        spec = spec,
        group = res.group,
        version = res.version,
        scope = res.scope,
    ));

    if has_status {
        code.push_str(&format!(
            "\x20       let status_schema = schemars::schema_for!({status});\n\
             \x20       map.insert(\"statusSchema\".into(), serde_json::to_value(status_schema).unwrap());\n",
        ));
    }

    code.push_str(
        "\x20       serde_json::Value::Object(map)\n\
         \x20   }\n\
         }\n",
    );

    code
}

/// Strip KapiResource-related imports and attributes from old-style api files
/// so they can be embedded in the helper project without depending on
/// `kapi-derive` or `kapi-controller`.
fn prepare_resource_module(content: &str) -> String {
    let re_import = Regex::new(r"(?m)^\s*use\s+(kapi_controller|kapi_derive).*;?\s*$").unwrap();
    let content = re_import.replace_all(content, "").to_string();

    // Remove #[derive(...KapiResource...)] lines.
    let re_derive = Regex::new(r"(?m)^\s*#\[derive\([^)]*KapiResource[^)]*\)\]\s*$").unwrap();
    let content = re_derive.replace_all(&content, "").to_string();

    // Remove #[kapi(...)] lines (including multi-line).
    let re_kapi = Regex::new(r"(?s)#\[kapi\([^)]*\)\]").unwrap();
    let content = re_kapi.replace_all(&content, "").to_string();

    // Clean up empty lines left behind.
    let re_blank = Regex::new(r"\n{3,}").unwrap();
    let content = re_blank.replace_all(&content, "\n\n").to_string();

    content.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capitalize_first() {
        assert_eq!(capitalize_first("widget"), "Widget");
        assert_eq!(capitalize_first(""), "");
        assert_eq!(capitalize_first("a"), "A");
        assert_eq!(capitalize_first("Namespace"), "Namespace");
    }

    #[test]
    fn test_prepare_resource_module_strips_kapi_import() {
        let input = r#"use kapi_controller::KapiResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, KapiResource, Serialize, Deserialize, JsonSchema)]
#[kapi(group = "io", version = "v1", kind = "Widget", scope = "Namespaced", status = "WidgetStatus")]
pub struct WidgetSpec {
    pub field1: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WidgetStatus {
    pub ready: bool,
}"#;

        let result = prepare_resource_module(input);

        // Old import should be stripped
        assert!(!result.contains("kapi_controller"), "import should be stripped");
        assert!(!result.contains("KapiResource"), "KapiResource should be stripped");
        assert!(!result.contains("#[kapi("), "kapi attr should be stripped");

        // Clean structs should remain
        assert!(result.contains("pub struct WidgetSpec"), "spec struct should remain");
        assert!(result.contains("pub struct WidgetStatus"), "status struct should remain");

        // schemars and serde imports should remain
        assert!(result.contains("use schemars::JsonSchema;"), "JsonSchema import should remain");
        assert!(
            result.contains("use serde::{Deserialize, Serialize};"),
            "serde import should remain"
        );
    }

    #[test]
    fn test_prepare_resource_module_clean_file() {
        // New-style api files (no KapiResource) should pass through mostly unchanged.
        let input = r#"use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// WidgetSpec defines the desired state of Widget.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WidgetSpec {
    pub field1: String,
}"#;

        let result = prepare_resource_module(input);
        assert!(result.contains("pub struct WidgetSpec"), "spec struct should remain");
        assert!(result.contains("use schemars::JsonSchema;"), "import should remain");
    }

    #[test]
    fn test_generate_wrapper_code_without_status() {
        let res = ResourceInfo {
            group: "io".into(),
            version: "v1".into(),
            kind: "Widget".into(),
            scope: "Namespaced".into(),
            spec_struct_name: "WidgetSpec".into(),
            status_struct_name: None,
            content: String::new(),
        };

        let code = generate_wrapper_code(&res);

        assert!(code.contains("pub struct Widget {"), "wrapper struct should be generated");
        assert!(code.contains("pub metadata: ObjectMeta"), "metadata field");
        assert!(code.contains("pub spec: WidgetSpec"), "spec field");
        assert!(!code.contains("pub status:"), "no status field");
        assert!(code.contains("pub fn key()"), "key method");
        assert!(code.contains("pub fn schema_data()"), "schema_data method");
        assert!(code.contains("\"Namespaced\""), "scope in schema_data");
    }

    #[test]
    fn test_generate_wrapper_code_with_status() {
        let res = ResourceInfo {
            group: "io".into(),
            version: "v1".into(),
            kind: "Gadget".into(),
            scope: "Cluster".into(),
            spec_struct_name: "GadgetSpec".into(),
            status_struct_name: Some("GadgetStatus".into()),
            content: String::new(),
        };

        let code = generate_wrapper_code(&res);

        assert!(code.contains("pub struct Gadget {"), "wrapper struct");
        assert!(code.contains("pub status: Option<GadgetStatus>"), "status field");
        assert!(code.contains("\"Cluster\""), "scope in schema_data");
        assert!(
            code.contains("let status_schema = schemars::schema_for!(GadgetStatus);"),
            "status schema"
        );
    }
}
