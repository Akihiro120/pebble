#[derive(Debug)]
pub enum AcquireError {
    Transient,
    Fatal(String),
}
