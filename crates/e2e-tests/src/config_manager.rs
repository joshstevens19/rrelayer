use crate::SigningProvider;
use anyhow::Result;
use tracing::info;

pub fn update_yaml_for_provider(_original_config: &str, provider: SigningProvider) -> Result<()> {
    let config_filename = format!("config/{}.yaml", provider.as_str());
    let config_path = std::path::Path::new(&config_filename);

    if !config_path.exists() {
        anyhow::bail!("Config file not found: {}", config_filename);
    }

    // Simply copy the provider-specific config file to rrelayer.yaml
    std::fs::copy(&config_path, "rrelayer.yaml")?;
    info!("✅ Configuration updated for {} (copied from {})", provider.as_str(), config_filename);

    Ok(())
}

pub fn ensure_default_config() -> Result<()> {
    let rrelayer_config_path = std::path::Path::new("rrelayer.yaml");

    if !rrelayer_config_path.exists() {
        info!("🔧 rrelayer.yaml not found, creating default config using raw provider");
        let default_config_path = std::path::Path::new("config/raw.yaml");

        if !default_config_path.exists() {
            anyhow::bail!("Default config file not found: config/raw.yaml");
        }

        std::fs::copy(default_config_path, "rrelayer.yaml")?;
        info!("✅ Created default rrelayer.yaml from config/raw.yaml");
    }

    Ok(())
}
