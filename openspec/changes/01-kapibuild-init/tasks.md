## 1. Project Setup

- [ ] 1.1 Verify kapibuild crate exists in workspace
- [ ] 1.2 Verify kapibuild has required dependencies: clap (with derive), serde, serde_json, serde_yaml, anyhow, thiserror

## 2. CLI Structure

- [ ] 2.1 Implement CLI argument parsing with clap (init subcommand)
- [ ] 2.2 Add --name flag for project name (optional, defaults to current directory)

## 3. Init Command Implementation

- [ ] 3.1 Implement directory structure creation (api/, schemas/, src/, src/controllers/)
- [ ] 3.2 Generate Cargo.toml template with all required dependencies
- [ ] 3.3 Generate Kapifile template with domain configuration
- [ ] 3.4 Generate src/main.rs template with Manager setup
- [ ] 3.5 Generate src/controllers/mod.rs template
- [ ] 3.6 Test `kapibuild init` creates valid project structure

## 4. Verification

- [ ] 4.1 Run `cargo clippy -p kapibuild` to check for linting issues
- [ ] 4.2 Test init command in a temporary directory
- [ ] 4.3 Verify generated Cargo.toml has correct dependencies
- [ ] 4.4 Verify generated main.rs compiles (syntax check)

## 5. Documentation

- [ ] 5.1 Create docs/kapibuild/project-structure.md describing the directory layout created by init
