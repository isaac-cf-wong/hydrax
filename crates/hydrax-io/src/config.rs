use std::{
    fmt::{Display, Formatter},
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;

/// Errors produced while reading a run-configuration file.
#[derive(Debug)]
pub enum ConfigurationError {
    /// The file could not be read.
    Unreadable {
        /// Path that was requested.
        path: PathBuf,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// The file was not valid TOML, or did not describe the expected
    /// configuration.
    Invalid {
        /// Path that was read.
        path: PathBuf,
        /// Underlying deserialization error, including the offending span.
        source: toml::de::Error,
    },
}

impl Display for ConfigurationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable { path, source } => write!(
                formatter,
                "cannot read run configuration {}: {source}",
                path.display()
            ),
            Self::Invalid { path, source } => write!(
                formatter,
                "run configuration {} is invalid: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ConfigurationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unreadable { source, .. } => Some(source),
            Self::Invalid { source, .. } => Some(source),
        }
    }
}

/// Read a run configuration from a TOML file.
///
/// The configuration type decides which keys exist and which are optional;
/// declaring it with `#[serde(deny_unknown_fields)]` turns a misspelled key
/// into an error naming the key instead of a silently ignored setting.
///
/// # Errors
///
/// Returns [`ConfigurationError::Unreadable`] if the file cannot be read and
/// [`ConfigurationError::Invalid`] if its contents are not valid TOML or do not
/// match `T`.
pub fn read_configuration<T: DeserializeOwned>(path: &Path) -> Result<T, ConfigurationError> {
    let text = fs::read_to_string(path).map_err(|source| ConfigurationError::Unreadable {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&text).map_err(|source| ConfigurationError::Invalid {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use serde::Deserialize;

    use super::{ConfigurationError, read_configuration};

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Configuration {
        frequency_hz: Option<f64>,
        label: Option<String>,
    }

    /// Create a private directory for one test's configuration files.
    fn temporary_directory(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "hydrax-io-configuration-{}-{name}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).expect("temporary directory is creatable");
        directory
    }

    /// Write one configuration file and return its path.
    fn write(directory: &Path, name: &str, contents: &str) -> PathBuf {
        let path = directory.join(name);
        std::fs::write(&path, contents).expect("configuration file is writable");
        path
    }

    #[test]
    fn declared_keys_are_read_and_absent_keys_stay_unset() {
        let directory = temporary_directory("declared");
        let path = write(&directory, "run.toml", "frequency_hz = 800.0\n");

        let configuration: Configuration =
            read_configuration(&path).expect("configuration is readable");

        assert!(
            (configuration.frequency_hz.expect("key is present") - 800.0).abs() < f64::EPSILON,
            "the file value must survive deserialization"
        );
        assert_eq!(configuration.label, None);

        std::fs::remove_dir_all(&directory).expect("temporary directory is removable");
    }

    #[test]
    fn a_missing_file_is_reported_with_its_path() {
        let directory = temporary_directory("missing");
        let path = directory.join("absent.toml");

        let error = read_configuration::<Configuration>(&path)
            .expect_err("a missing file must be rejected");

        assert!(matches!(error, ConfigurationError::Unreadable { .. }));
        assert!(
            error.to_string().contains("absent.toml"),
            "the message must name the file, got {error}"
        );
        assert!(
            std::error::Error::source(&error).is_some(),
            "the filesystem error must remain reachable"
        );

        std::fs::remove_dir_all(&directory).expect("temporary directory is removable");
    }

    #[test]
    fn an_unknown_key_is_rejected_by_name() {
        let directory = temporary_directory("unknown");
        let path = write(&directory, "run.toml", "frequenzy_hz = 800.0\n");

        let error = read_configuration::<Configuration>(&path)
            .expect_err("an unknown key must be rejected");

        assert!(matches!(error, ConfigurationError::Invalid { .. }));
        let message = error.to_string();
        assert!(
            message.contains("frequenzy_hz"),
            "the message must name the offending key, got {message}"
        );
        assert!(
            message.contains("run.toml"),
            "the message must name the file, got {message}"
        );

        std::fs::remove_dir_all(&directory).expect("temporary directory is removable");
    }

    #[test]
    fn a_value_of_the_wrong_type_is_rejected() {
        let directory = temporary_directory("wrong-type");
        let path = write(&directory, "run.toml", "frequency_hz = \"loud\"\n");

        let error = read_configuration::<Configuration>(&path)
            .expect_err("a mistyped value must be rejected");

        assert!(matches!(error, ConfigurationError::Invalid { .. }));
        assert!(
            std::error::Error::source(&error).is_some(),
            "the deserialization error must remain reachable"
        );

        std::fs::remove_dir_all(&directory).expect("temporary directory is removable");
    }

    #[test]
    fn malformed_toml_is_rejected() {
        let directory = temporary_directory("malformed");
        let path = write(&directory, "run.toml", "frequency_hz =\n");

        let error = read_configuration::<Configuration>(&path)
            .expect_err("malformed TOML must be rejected");

        assert!(matches!(error, ConfigurationError::Invalid { .. }));

        std::fs::remove_dir_all(&directory).expect("temporary directory is removable");
    }
}
