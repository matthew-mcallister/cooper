use crate::*;

pub type Version = (u8, u8);

/// Reflection info for a shader module.
#[derive(Debug, Default)]
pub struct ShaderModule {
    /// The SPIR-V version as a pair `(major, minor)`.
    pub version: Version,
    pub entry_points: Vec<EntryPoint>,
    pub source_language: Option<SourceLanguage>,
    pub source_language_version: u32,
    pub source_file: Option<String>,
    pub source_source: Option<String>,
}

#[derive(Debug, Default)]
pub struct EntryPoint {
    pub execution_model: ExecutionModel,
    pub name: String,
    /// A list of variables used by the entry point.
    pub interface: Vec<u32>,
}
