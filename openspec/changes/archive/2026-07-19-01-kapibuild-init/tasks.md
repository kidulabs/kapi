## 1. Project Setup

- [x] 1.1 Verify kapibuild crate exists in workspace
- [x] 1.2 Verify kapibuild has required dependencies: clap (with derive), serde, serde_json, serde_yaml, anyhow, thiserror

## 2. CLI Structure

- [x] 2.1 Implement CLI argument parsing with clap (init subcommand)
- [x] 2.2 Add --name flag for project name (optional, defaults to current directory)

## 3. Init Command Implementation

- [x] 3.1 Implement directory structure creation (api/, schemas/, src/, src/controllers/)
- [x] 3.2 Generate Cargo.toml template with all required dependencies
- [x] 3.3 Generate Kapifile template with domain configuration
- [x] 3.4 Generate src/main.rs template with Manager setup
- [x] 3.5 Generate src/controllers/mod.rs template
- [x] 3.6 Test `kapibuild init` creates valid project structure

## 4. Verification

- [x] 4.1 Run `cargo clippy -p kapibuild` to check for linting issues
- [x] 4.2 Test init command in a temporary directory
- [x] 4.3 Verify generated Cargo.toml has correct dependencies
- [x] 4.4 Verify generated main.rs compiles (syntax check)

## 5. Documentation

- [x] 5.1 Create docs/kapibuild/project-structure.md describing the directory layout created by init
