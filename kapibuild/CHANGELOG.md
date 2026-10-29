# Changelog

All notable changes to `kapibuild` are documented here.

## [0.5.1]

### Fixed

- Multi-word (camelCase) kinds no longer fail to resolve from api file names.
  `kapibuild` used to derive a resource kind by uppercasing only the first
  character of the file stem, so `src/api/kcloud.io/v1/virtualmachine.rs`
  produced `Virtualmachine` and matched neither the Kapifile registration
  (`kind: VirtualMachine`) nor the `VirtualMachineSpec`/`VirtualMachineStatus`
  structs. Resolution is now case-insensitive on both sides and the Kapifile
  registration is authoritative for the kind string, the `{Kind}Spec` /
  `{Kind}Status` struct names, and all generated output.
- Auto-discovery in `kapibuild controller generate` uses the same resolution
  rule, so controllers are discovered for camelCase kinds too.
- Unregistered and ambiguous api files are reported as a single aggregated
  error listing every problem file, instead of stopping at the first one.
  Ambiguous collisions (two registered kinds whose lowercase forms are equal)
  are always reported, never guessed.
- No output changes for previously-working kinds: for every input that resolved
  before, the registered kind equals the old capitalized stem, so generated
  files are byte-identical.
