#[derive(thiserror::Error, Debug)]
pub enum CoreError {
    #[error("invalid field selector: {0}")]
    InvalidFieldSelector(String),

    #[error("invalid label selector: {0}")]
    InvalidLabelSelector(String),
}
