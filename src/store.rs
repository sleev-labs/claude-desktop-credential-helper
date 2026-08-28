use std::fmt;

#[derive(Debug)]
pub struct Payload {
    pub source: String,
    pub json: String,
}

#[derive(Debug)]
pub enum StoreError {
    NotFound { location: String },
    Unreadable { location: String, cause: String },
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { location } => write!(f, "no credentials at {location}"),
            Self::Unreadable { location, cause } => {
                write!(f, "could not read {location}: {cause}")
            }
        }
    }
}

pub fn read() -> Result<Payload, StoreError> {
    platform::read()
}

#[cfg(target_os = "macos")]
mod platform {
    use std::process::Command;

    use super::{Payload, StoreError};

    const SERVICE: &str = "Claude Code-credentials";

    /// `security(1)` exit status for errSecItemNotFound.
    const NOT_FOUND_STATUS: i32 = 44;

    pub fn read() -> Result<Payload, StoreError> {
        let location = format!("the macOS Keychain (service \"{SERVICE}\")");
        let unreadable = |cause: String| StoreError::Unreadable {
            location: location.clone(),
            cause,
        };

        let output = Command::new("security")
            .args(["find-generic-password", "-s", SERVICE, "-w"])
            .output()
            .map_err(|error| unreadable(format!("failed to run `security`: {error}")))?;
        if output.status.code() == Some(NOT_FOUND_STATUS) {
            return Err(StoreError::NotFound { location });
        }
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(unreadable(format!(
                "`security` failed ({}): {}",
                output.status,
                stderr.trim()
            )));
        }
        let json = String::from_utf8(output.stdout)
            .map_err(|_| unreadable("keychain item is not valid UTF-8".to_owned()))?;
        Ok(Payload {
            source: location,
            json: json.trim_end().to_owned(),
        })
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use super::{Payload, StoreError};

    const FILE_NAME: &str = ".credentials.json";

    pub fn read() -> Result<Payload, StoreError> {
        let path = config_dir(std::env::var_os("CLAUDE_CONFIG_DIR"), std::env::home_dir())
            .ok_or_else(|| StoreError::NotFound {
                location: format!("~/.claude/{FILE_NAME} (no home directory)"),
            })?
            .join(FILE_NAME);
        let location = path.display().to_string();
        match std::fs::read_to_string(&path) {
            Ok(json) => Ok(Payload {
                source: location,
                json,
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Err(StoreError::NotFound { location })
            }
            Err(error) => Err(StoreError::Unreadable {
                location,
                cause: error.to_string(),
            }),
        }
    }

    fn config_dir(config_dir: Option<OsString>, home: Option<PathBuf>) -> Option<PathBuf> {
        match config_dir {
            Some(dir) if !dir.is_empty() => Some(PathBuf::from(dir)),
            _ => Some(home?.join(".claude")),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn config_dir_env_takes_precedence() {
            let dir = config_dir(Some("/custom".into()), Some(PathBuf::from("/home/u")));
            assert_eq!(dir, Some(PathBuf::from("/custom")));
        }

        #[test]
        fn empty_env_falls_back_to_home() {
            let dir = config_dir(Some(OsString::new()), Some(PathBuf::from("/home/u")));
            assert_eq!(dir, Some(PathBuf::from("/home/u/.claude")));
        }

        #[test]
        fn no_home_and_no_env_is_none() {
            assert_eq!(config_dir(None, None), None);
        }
    }
}
