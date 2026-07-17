/// Errors that can occur when acquiring a frame from the backend.
#[derive(Debug)]
pub enum AcquireError {
    /// The acquire failed temporarily. The frame is skipped but rendering can
    /// continue on the next tick (e.g. the swapchain is out of date).
    Transient,
    /// An unrecoverable error occurred. Includes a human-readable description.
    Fatal(String),
}
