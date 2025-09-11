use crate::commands::error::ProjectLocationError;
use rrelayer_core::{SetupConfig, read};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ProjectLocation {
    output_dir: PathBuf,
    override_project_name: Option<String>,
}

impl ProjectLocation {
    /// Creates a new ProjectLocation instance.
    ///
    /// # Arguments
    /// * `output_dir` - The directory where keystores and configuration will be stored
    ///
    /// # Returns
    /// * A new ProjectLocation instance
    pub fn new(output_dir: PathBuf) -> Self {
        Self { output_dir, override_project_name: None }
    }

    /// Overrides the project name for this project location.
    ///
    /// # Arguments
    /// * `name` - The name to override the project name with
    pub fn override_project_name(&mut self, name: &str) {
        self.override_project_name = Some(name.to_string());
    }

    /// Reads and parses the setup configuration from the rrelayer.yaml file.
    ///
    /// # Arguments
    /// * `raw_yaml` - Whether to read the YAML file as raw text
    ///
    /// # Returns
    /// * `Ok(SetupConfig)` - Successfully parsed configuration
    /// * `Err(ProjectLocationError)` - Failed to read or parse configuration
    pub fn setup_config(&self, raw_yaml: bool) -> Result<SetupConfig, ProjectLocationError> {
        let yaml = read(&self.output_dir.join("rrelayer.yaml"), raw_yaml).map_err(|e| {
            ProjectLocationError::ProjectConfig(format!("Failed to read config: {}", e))
        })?;
        Ok(yaml)
    }

    /// Overwrites the setup configuration file with the provided configuration.
    ///
    /// # Arguments
    /// * `config` - The setup configuration to write
    ///
    /// # Returns
    /// * `Ok(())` - Configuration written successfully
    /// * `Err(ProjectLocationError)` - Failed to serialize or write configuration
    pub fn overwrite_setup_config(&self, config: SetupConfig) -> Result<(), ProjectLocationError> {
        let yaml = serde_yaml::to_string(&config)?;
        fs::write(&self.output_dir.join("rrelayer.yaml"), yaml)?;
        Ok(())
    }

    /// Gets the API URL from the setup configuration.
    ///
    /// # Returns
    /// * `Ok(String)` - The API URL (http://localhost:port)
    /// * `Err(ProjectLocationError)` - Failed to read configuration
    pub fn get_api_url(&self) -> Result<String, ProjectLocationError> {
        let setup_config = self.setup_config(false)?;
        Ok(format!("http://localhost:{}", setup_config.api_config.port))
    }
}
