#[derive(Debug, Clone, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResourceKey {
    pub group: String,
    pub version: String,
    pub kind: String,
}
