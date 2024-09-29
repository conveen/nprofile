/// Crate-level error type.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Command did not exit successfully.
    #[error("Command exited with code {code}: {message}")]
    CommandFailure { code: i32, message: String },
    /// Command formatting errors.
    #[error(transparent)]
    Format(#[from] interpolator::Error),
    /// Environment is not defined for a profile.
    #[error("Environment {environment} not defined for profile {profile}")]
    InvalidEnvironment { environment: String, profile: String },
    /// IO errors.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Profile requirements not met.
    #[error("Profile requirements not met: {message}")]
    ProfileRequirementsNotMet { message: String },
    /// Error from creating an `&str` from `&[u8]`.
    #[error(transparent)]
    Utf8(#[from] std::str::Utf8Error),
}

/// Crate-level result type that wraps [Error](enum.Error.html).
pub type Result<T> = std::result::Result<T, Error>;
