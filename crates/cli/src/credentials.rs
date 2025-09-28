use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn get_storage_dir() -> Result<PathBuf, CredentialError> {
    let home_dir = std::env::var("HOME").map_err(|_| CredentialError::NotFound)?;
    let storage_dir = PathBuf::from(home_dir).join(".rrelayer");
    if !storage_dir.exists() {
        fs::create_dir_all(&storage_dir)
            .map_err(|e| CredentialError::Io(format!("Failed to create directory: {}", e)))?;
    }
    Ok(storage_dir)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredentials {
    pub api_url: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug)]
pub enum CredentialError {
    Io(String),
    Json(serde_json::Error),
    NotFound,
}

impl From<serde_json::Error> for CredentialError {
    fn from(err: serde_json::Error) -> Self {
        CredentialError::Json(err)
    }
}

impl std::fmt::Display for CredentialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CredentialError::Io(err) => write!(f, "IO error: {}", err),
            CredentialError::Json(err) => write!(f, "JSON error: {}", err),
            CredentialError::NotFound => write!(f, "Credentials not found"),
        }
    }
}

impl std::error::Error for CredentialError {}

pub fn store_credentials(
    profile_name: &str,
    credentials: &StoredCredentials,
) -> Result<(), CredentialError> {
    let storage_dir = get_storage_dir()?;
    let file_path = storage_dir.join(format!("{}.json", profile_name));
    let json_data = serde_json::to_string_pretty(credentials)?;
    fs::write(file_path, json_data)
        .map_err(|e| CredentialError::Io(format!("Failed to write credentials: {}", e)))?;
    Ok(())
}

pub fn load_credentials(profile_name: &str) -> Result<StoredCredentials, CredentialError> {
    let storage_dir = get_storage_dir()?;
    let file_path = storage_dir.join(format!("{}.json", profile_name));
    if !file_path.exists() {
        return Err(CredentialError::NotFound);
    }
    let json_data = fs::read_to_string(file_path)
        .map_err(|e| CredentialError::Io(format!("Failed to read credentials: {}", e)))?;
    let credentials: StoredCredentials = serde_json::from_str(&json_data)?;
    Ok(credentials)
}

pub fn delete_credentials(profile_name: &str) -> Result<(), CredentialError> {
    let storage_dir = get_storage_dir()?;
    let file_path = storage_dir.join(format!("{}.json", profile_name));
    if file_path.exists() {
        fs::remove_file(file_path)
            .map_err(|e| CredentialError::Io(format!("Failed to delete credentials: {}", e)))?;
    }
    Ok(())
}

pub fn list_profiles() -> Result<Vec<String>, CredentialError> {
    let mut profiles = Vec::new();
    let storage_dir = get_storage_dir()?;

    if storage_dir.exists() {
        if let Ok(entries) = fs::read_dir(storage_dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_file() && path.extension() == Some(std::ffi::OsStr::new("json")) {
                        if let Some(stem) = path.file_stem() {
                            if let Some(profile_name) = stem.to_str() {
                                profiles.push(profile_name.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    profiles.sort();
    Ok(profiles)
}

pub fn add_profile_to_list(_profile_name: &str) -> Result<(), CredentialError> {
    // No-op for file-based storage - profiles are auto-discovered
    Ok(())
}

pub fn remove_profile_from_list(_profile_name: &str) -> Result<(), CredentialError> {
    // No-op for file-based storage - profiles are auto-discovered
    Ok(())
}
