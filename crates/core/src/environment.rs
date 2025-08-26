use std::path::Path;

use dotenv::{dotenv, from_path};

/// Loads environment variables from a .env file in the project directory.
///
/// First attempts to load a .env file from the specified project path.
/// If that fails (file doesn't exist or cannot be read), falls back to
/// loading from the current working directory or system environment.
///
/// This function allows for project-specific environment configurations
/// while maintaining backward compatibility.
///
/// # Arguments
/// * `project_path` - Path to the project directory to search for .env file
pub fn load_env_from_project_path(project_path: &Path) {
    if from_path(project_path.join(".env")).is_err() {
        dotenv().ok();
    }
}
