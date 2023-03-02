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

    #[error("Could not symlink {0} to {1}: {2}")]
    /// Occurs when there is a problem symlinking a file or a directory
    Symlink(String, String, String),

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

    #[error("Could not find repository for {0}")]
    /// Occurs when the repository for shaders or presets cannot be found
    RepositoryNotFound(String),
    #[error("Could not find branch {0} for repository {1}")]
    /// Occurs when the branch for shaders or presets cannot be found
    BranchNotFound(String, String),
    #[error("Merge conflicts found for branch {0} of repository {1}")]
    /// Occurs when the branch for shaders or presets cannot be merged
    MergeConflict(String, String),

    #[error(transparent)]
    /// Forwards the errors from `std::io::Error`
    Io(#[from] std::io::Error),

    #[error(transparent)]
    /// Forwards the errors from `reqwest::Error`
    Reqwest(#[from] reqwest::Error),

    #[error(transparent)]
    /// Forwards the errors from `git2::Error`
    Git(#[from] git2::Error),

    #[error(transparent)]
    /// Forwards the errors from `zip::result::ZipError`
    Zip(#[from] zip::result::ZipError),
}

impl From<ReShaderError> for inquire::InquireError {
    fn from(value: ReShaderError) -> Self {
        inquire::InquireError::Custom(Box::new(value))
    }
}

/// A type alias for `Result<T, ReShaderError>`
pub type ReShaderResult<T> = std::result::Result<T, ReShaderError>;
