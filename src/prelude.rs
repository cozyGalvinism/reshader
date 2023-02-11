#[derive(thiserror::Error, Debug)]
pub enum ReShaderError {
    #[error("Unable to fetch latest ReShade version: {0}")]
    FetchLatestVersion(String),

    #[error("Unable to download {0}: {1}")]
    Download(String, String),

    #[error("ReShade installer had no zip file")]
    NoZipFile,

    #[error("ReShade64.dll not found in ReShade installer")]
    NoReShade64Dll,

    #[error("Unable to read zip file")]
    ReadZipFile,
    #[error("Unable to extract zip file")]
    ExtractZipFile,
}

impl From<ReShaderError> for inquire::InquireError {
    fn from(value: ReShaderError) -> Self {
        inquire::InquireError::Custom(Box::new(value))
    }
}
