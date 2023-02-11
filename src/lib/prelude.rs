#[derive(thiserror::Error, Debug)]
/// Errors that can occur when installing ReShade
pub enum ReShaderError {
    #[error("Unable to fetch latest ReShade version: {0}")]
    /// Occurs when there is a problem fetching the latest ReShade version from GitHub
    ///
    /// For example, if the GitHub API is down, this error will occur.
    FetchLatestVersion(String),

    #[error("Unable to download {0}: {1}")]
    /// Occurs when there is a problem downloading a file
    ///
    /// Additionally, the second argument is additional information about the error.
    Download(String, String),

    #[error("ReShade installer had no zip file")]
    /// Occurs when the ReShade installer doesn't have a zip file (it's missing its byte sequence for the zip file)
    NoZipFile,

    #[error("ReShade64.dll not found in ReShade installer")]
    /// Occurs when the ReShade installer doesn't have a ReShade64.dll file contained in it
    NoReShade64Dll,

    #[error("Unable to read zip file")]
    /// Occurs when the ReShade installer's zip file cannot be read
    ReadZipFile,
    #[error("Unable to extract zip file")]
    /// Occurs when the ReShade installer's zip file cannot be extracted
    ExtractZipFile,

    #[error(transparent)]
    /// Forwards the errors from `std::io::Error`
    Io(#[from] std::io::Error),

    #[error(transparent)]
    /// Forwards the errors from `reqwest::Error`
    Reqwest(#[from] reqwest::Error),
}

impl From<ReShaderError> for inquire::InquireError {
    fn from(value: ReShaderError) -> Self {
        inquire::InquireError::Custom(Box::new(value))
    }
}

/// A type alias for `Result<T, ReShaderError>`
pub type ReShaderResult<T> = std::result::Result<T, ReShaderError>;
