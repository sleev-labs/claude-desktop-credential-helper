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
    Unwritable { location: String, cause: String },
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { location } => write!(f, "no credentials at {location}"),
            Self::Unreadable { location, cause } => {
                write!(f, "could not read {location}: {cause}")
            }
            Self::Unwritable { location, cause } => {
                write!(f, "could not write {location}: {cause}")
            }
        }
    }
}

pub fn read() -> Result<Payload, StoreError> {
    platform::read()
}

pub fn write(json: &str) -> Result<(), StoreError> {
    platform::write(json)
}

#[cfg(target_os = "macos")]
mod platform {
    use std::process::Command;

    use super::{Payload, StoreError};

    const SERVICE: &str = "Claude Code-credentials";

    /// `security(1)` exit status for errSecItemNotFound.
    const NOT_FOUND_STATUS: i32 = 44;

    fn location() -> String {
        format!("the macOS Keychain (service \"{SERVICE}\")")
    }

    pub fn read() -> Result<Payload, StoreError> {
        let location = location();
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

    pub fn write(json: &str) -> Result<(), StoreError> {
        let location = location();
        let unwritable = |cause: String| StoreError::Unwritable {
            location: location.clone(),
            cause,
        };

        let account = account(&unwritable)?;
        // `security` has no way to take the secret on stdin, so it is visible
        // to other processes of this user for the life of the call.
        let output = Command::new("security")
            .args([
                "add-generic-password",
                "-U",
                "-s",
                SERVICE,
                "-a",
                &account,
                "-w",
                json,
            ])
            .output()
            .map_err(|error| unwritable(format!("failed to run `security`: {error}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(unwritable(format!(
                "`security` failed ({}): {}",
                output.status,
                stderr.trim()
            )));
        }
        Ok(())
    }

    fn account(unwritable: &impl Fn(String) -> StoreError) -> Result<String, StoreError> {
        let output = Command::new("security")
            .args(["find-generic-password", "-s", SERVICE])
            .output()
            .map_err(|error| unwritable(format!("failed to run `security`: {error}")))?;
        if !output.status.success() {
            return Err(unwritable("the keychain item has no account".to_owned()));
        }
        let attributes = String::from_utf8_lossy(&output.stdout);
        super::parse_account(&attributes)
            .ok_or_else(|| unwritable("the keychain item has no account".to_owned()))
    }
}

/// Pull `acct` out of `security find-generic-password` attribute output.
#[cfg(any(target_os = "macos", test))]
fn parse_account(attributes: &str) -> Option<String> {
    let (_, rest) = attributes.split_once("\"acct\"<blob>=\"")?;
    let (account, _) = rest.split_once('"')?;
    Some(account.to_owned())
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    use super::{Payload, StoreError};

    const FILE_NAME: &str = ".credentials.json";

    pub fn read() -> Result<Payload, StoreError> {
        let path = store_path()?;
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

    pub fn write(json: &str) -> Result<(), StoreError> {
        let path = store_path()?;
        write_file(&path, json).map_err(|cause| StoreError::Unwritable {
            location: path.display().to_string(),
            cause,
        })
    }

    /// Write through a sibling temp file so a failure cannot leave the store
    /// half-written; the rename is atomic and replaces the old file.
    fn write_file(path: &Path, json: &str) -> Result<(), String> {
        use std::io::Write;

        let directory = path.parent().ok_or("the store has no parent directory")?;
        let temporary = directory.join(format!("{FILE_NAME}.{}.tmp", std::process::id()));

        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }

        let outcome = options
            .open(&temporary)
            .and_then(|mut file| {
                file.write_all(json.as_bytes())
                    .and_then(|()| file.sync_all())
            })
            .and_then(|()| std::fs::rename(&temporary, path));
        if let Err(error) = outcome {
            let _ = std::fs::remove_file(&temporary);
            return Err(error.to_string());
        }
        Ok(())
    }

    fn store_path() -> Result<PathBuf, StoreError> {
        let directory = config_dir(std::env::var_os("CLAUDE_CONFIG_DIR"), std::env::home_dir())
            .ok_or_else(|| StoreError::NotFound {
                location: format!("~/.claude/{FILE_NAME} (no home directory)"),
            })?;
        Ok(directory.join(FILE_NAME))
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

        #[test]
        fn write_file_replaces_the_store() {
            let directory = std::env::temp_dir().join(format!("cdc-{}", std::process::id()));
            std::fs::create_dir_all(&directory).unwrap();
            let path = directory.join(FILE_NAME);

            write_file(&path, "{\"a\":1}").unwrap();
            write_file(&path, "{\"a\":2}").unwrap();

            assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"a\":2}");
            assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
            std::fs::remove_dir_all(&directory).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_the_keychain_account() {
        let attributes = "keychain: \"/Users/u/Library/Keychains/login.keychain-db\"\n    \"acct\"<blob>=\"u\"\n    \"svce\"<blob>=\"Claude Code-credentials\"\n";
        assert_eq!(super::parse_account(attributes).as_deref(), Some("u"));
        assert_eq!(super::parse_account("no account here"), None);
    }
}
