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
    pub fn new(output_dir: PathBuf) -> Self {
        Self { output_dir, override_project_name: None }
    }

    pub fn override_project_name(&mut self, name: &str) {
        self.override_project_name = Some(name.to_string());
    }

    pub fn setup_config(&self, raw_yaml: bool) -> Result<SetupConfig, ProjectLocationError> {
        let yaml = read(&self.output_dir.join("rrelayer.yaml"), raw_yaml).map_err(|e| {
            ProjectLocationError::ProjectConfig(format!("Failed to read config: {}", e))
        })?;
        Ok(yaml)
    }

    pub fn overwrite_setup_config(&self, config: SetupConfig) -> Result<(), ProjectLocationError> {
        let yaml = serde_yaml::to_string(&config)?;
        fs::write(self.output_dir.join("rrelayer.yaml"), yaml)?;
        Ok(())
    }

    pub fn get_api_url(&self) -> Result<String, ProjectLocationError> {
        let setup_config = self.setup_config(false)?;
        Ok(format!(
            "http://{}:{}",
            setup_config.api_config.host.unwrap_or("localhost".to_string()),
            setup_config.api_config.port
        ))
    }
}
