//! Manifest schema shared between build and host tooling.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const MANIFEST_FORMAT_VERSION: u32 = 1;

/// Build profile recorded in the manifest for reproducibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    Debug,
    Release,
}

impl Profile {
    pub fn as_str(self) -> &'static str {
        match self {
            Profile::Debug => "debug",
            Profile::Release => "release",
        }
    }
}

/// Serialized manifest describing the build artifacts for a guest program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub format_version: u32,
    pub codec_version: u32,
    pub bin_name: String,
    pub target: Option<String>,
    pub profile: Profile,
    #[serde(alias = "app_bin")]
    pub bin_file: String,
    #[serde(alias = "app_elf")]
    pub elf_file: String,
    #[serde(alias = "app_text")]
    pub text_file: String,
    #[serde(default)]
    pub bin_sha256: String,
}

/// Errors returned by manifest read, write, and parse operations.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse manifest: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("failed to serialize manifest: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("unsupported format_version {0}")]
    UnsupportedFormatVersion(u32),
}

impl Manifest {
    /// Read a manifest from a TOML file.
    pub fn read_from_file(path: &Path) -> Result<Self, ManifestError> {
        let content = fs::read_to_string(path)?;
        Self::parse(&content)
    }

    /// Write this manifest to a TOML file.
    pub fn write_to_file(&self, path: &Path) -> Result<(), ManifestError> {
        let payload = self.to_toml()?;
        fs::write(path, payload)?;
        Ok(())
    }

    /// Parse and validate a manifest from TOML text.
    pub fn parse(content: &str) -> Result<Self, ManifestError> {
        let manifest: Self = toml::from_str(content)?;
        if manifest.format_version != MANIFEST_FORMAT_VERSION {
            return Err(ManifestError::UnsupportedFormatVersion(
                manifest.format_version,
            ));
        }
        Ok(manifest)
    }

    /// Serialize this manifest to TOML text.
    pub fn to_toml(&self) -> Result<String, ManifestError> {
        Ok(toml::to_string(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_roundtrip() {
        let manifest = Manifest {
            format_version: MANIFEST_FORMAT_VERSION,
            codec_version: 0,
            bin_name: "demo".to_string(),
            target: None,
            profile: Profile::Release,
            bin_file: "app.bin".to_string(),
            elf_file: "app.elf".to_string(),
            text_file: "app.text".to_string(),
            bin_sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                .to_string(),
        };
        let toml = manifest.to_toml().expect("serialize");
        assert!(toml.contains("bin_file"));
        assert!(toml.contains("elf_file"));
        assert!(toml.contains("text_file"));
        assert!(toml.contains("bin_sha256"));
        assert!(!toml.contains("app_bin"));
        assert!(!toml.contains("app_elf"));
        assert!(!toml.contains("app_text"));
        let parsed = Manifest::parse(&toml).expect("parse");
        assert_eq!(parsed, manifest);
    }

    #[test]
    fn rejects_unknown_format_version() {
        let mut manifest = Manifest {
            format_version: MANIFEST_FORMAT_VERSION,
            codec_version: 0,
            bin_name: "demo".to_string(),
            target: None,
            profile: Profile::Release,
            bin_file: "app.bin".to_string(),
            elf_file: "app.elf".to_string(),
            text_file: "app.text".to_string(),
            bin_sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                .to_string(),
        };
        manifest.format_version += 1;
        let toml = manifest.to_toml().expect("serialize");
        let err = Manifest::parse(&toml).expect_err("error");
        assert!(matches!(err, ManifestError::UnsupportedFormatVersion(_)));
    }

    #[test]
    fn parses_legacy_artifact_field_names() {
        let legacy = r#"
format_version = 1
codec_version = 0
bin_name = "demo"
profile = "release"
app_bin = "app.bin"
app_elf = "app.elf"
app_text = "app.text"
"#;
        let manifest = Manifest::parse(legacy).expect("parse legacy manifest");
        assert_eq!(manifest.bin_file, "app.bin");
        assert_eq!(manifest.elf_file, "app.elf");
        assert_eq!(manifest.text_file, "app.text");
        assert_eq!(manifest.bin_sha256, "");
    }
}
