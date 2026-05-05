pub mod memory;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct ResourceKey {
    pub group: String,
    pub version: String,
    pub kind: String,
}
