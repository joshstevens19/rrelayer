use std::{fs, path::PathBuf, str::FromStr};

use alloy::signers::local::{LocalSigner, MnemonicBuilder, coins_bip39::English};
use clap::Subcommand;
use dialoguer::Password;
use rrelayer_core::{
    SetupConfig,
    keystore::{
        KeyStorePasswordManager, KeystoreDecryptResult, create_new_mnemonic_in_keystore,
        create_new_private_key_in_keystore, decrypt_keystore, store_mnemonic_in_keystore,
        store_private_key_in_keystore,
    },
    read,
};

use crate::commands::error::KeystoreError;

#[derive(Subcommand)]
pub enum KeystoreCommand {
    /// Create a new keystore from a mnemonic phrase
    CreateFromMnemonic {
        /// Use an existing mnemonic phrase
        #[clap(long)]
        mnemonic: Option<String>,

        /// Generate a new random mnemonic phrase
        #[clap(long, conflicts_with = "mnemonic")]
        generate: bool,

        /// Account name/profile for the keystore
        #[clap(long, default_value = "default")]
        name: String,

        /// Custom output directory (defaults to global config dir)
        #[clap(long)]
        output_dir: Option<PathBuf>,
    },

    /// Create a new keystore from a private key
    CreateFromPrivateKey {
        /// Private key (with or without 0x prefix)
        #[clap(long)]
        private_key: Option<String>,

        /// Generate a new random private key
        #[clap(long, conflicts_with = "private_key")]
        generate: bool,

        /// Account name/profile for the keystore
        #[clap(long, default_value = "default")]
        name: String,

        /// Custom output directory (defaults to place the cli was executed)
        #[clap(long)]
        output_dir: Option<PathBuf>,
    },

    /// Decrypt a keystore file to view its contents
    Decrypt {
        /// Path to the keystore file
        #[clap(long)]
        path: PathBuf,
    },
}

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

    /// Returns the path to the keystores directory.
    ///
    /// # Returns
    /// * Path to the keystores directory within the output directory
    fn get_keystore_dir(&self) -> PathBuf {
        self.output_dir.join("keystores")
    }

    /// Checks if a keystore with the given name already exists.
    ///
    /// # Arguments
    /// * `name` - The name of the keystore to check
    ///
    /// # Returns
    /// * `true` if the keystore exists, `false` otherwise
    fn keystore_already_exists(&self, name: &str) -> bool {
        self.get_keystore_dir().join(name).exists()
    }

    /// Returns the path to the account keystores directory.
    ///
    /// # Returns
    /// * Path to the account keystores directory
    fn get_account_keystore_dir(&self) -> PathBuf {
        self.output_dir.join("keystores").join("accounts")
    }

    /// Gets the path to a specific account keystore.
    ///
    /// # Arguments
    /// * `account` - The account name
    ///
    /// # Returns
    /// * Path to the specified account's keystore
    pub fn get_account_keystore(&self, account: &str) -> PathBuf {
        self.output_dir.join("keystores").join("accounts").join(account)
    }

    /// Creates the keystore directory if it doesn't exist.
    ///
    /// # Returns
    /// * `Ok(())` - Directory created successfully or already exists
    /// * `Err(KeystoreError)` - Failed to create directory
    fn create_keystore_dir(&self) -> Result<(), KeystoreError> {
        fs::create_dir_all(&self.get_keystore_dir())?;
        Ok(())
    }

    /// Creates the account keystore directory if it doesn't exist.
    ///
    /// # Returns
    /// * `Ok(())` - Directory created successfully or already exists
    /// * `Err(KeystoreError)` - Failed to create directory
    fn create_account_keystore_dir(&self) -> Result<(), KeystoreError> {
        fs::create_dir_all(&self.get_account_keystore_dir())?;
        Ok(())
    }

    /// Checks if an account with the given name already exists.
    ///
    /// # Arguments
    /// * `name` - The account name to check
    ///
    /// # Returns
    /// * `true` if the account exists, `false` otherwise
    fn account_already_exists(&self, name: &str) -> bool {
        self.get_account_keystore_dir().join(name).exists()
    }

    /// Reads and parses the setup configuration from the rrelayer.yaml file.
    ///
    /// # Arguments
    /// * `raw_yaml` - Whether to read the YAML file as raw text
    ///
    /// # Returns
    /// * `Ok(SetupConfig)` - Successfully parsed configuration
    /// * `Err(KeystoreError)` - Failed to read or parse configuration
    pub fn setup_config(&self, raw_yaml: bool) -> Result<SetupConfig, KeystoreError> {
        let yaml = read(&self.output_dir.join("rrelayer.yaml"), raw_yaml)
            .map_err(|e| KeystoreError::ProjectConfig(format!("Failed to read config: {}", e)))?;
        Ok(yaml)
    }

    /// Overwrites the setup configuration file with the provided configuration.
    ///
    /// # Arguments
    /// * `config` - The setup configuration to write
    ///
    /// # Returns
    /// * `Ok(())` - Configuration written successfully
    /// * `Err(KeystoreError)` - Failed to serialize or write configuration
    pub fn overwrite_setup_config(&self, config: SetupConfig) -> Result<(), KeystoreError> {
        let yaml = serde_yaml::to_string(&config)?;
        fs::write(&self.output_dir.join("rrelayer.yaml"), yaml)?;
        Ok(())
    }

    /// Gets the project name, either from override or from the configuration file.
    ///
    /// # Returns
    /// * The project name, or "unknown_project" if unable to determine
    pub fn get_project_name(&self) -> String {
        self.override_project_name.clone().unwrap_or_else(|| {
            self.setup_config(false)
                .map(|config| config.name)
                .unwrap_or_else(|_| "unknown_project".to_string())
        })
    }

    /// Gets the API URL from the setup configuration.
    ///
    /// # Returns
    /// * `Ok(String)` - The API URL (http://localhost:port)
    /// * `Err(KeystoreError)` - Failed to read configuration
    pub fn get_api_url(&self) -> Result<String, KeystoreError> {
        let setup_config = self.setup_config(false)?;
        Ok(format!("http://localhost:{}", setup_config.api_config.port))
    }
}

/// Handles keystore commands by dispatching to the appropriate handler function.
///
/// # Arguments
/// * `cmd` - The keystore command to execute
///
/// # Returns
/// * `Ok(())` - Command executed successfully
/// * `Err(KeystoreError)` - Command execution failed
pub async fn handle_keystore_command(cmd: &KeystoreCommand) -> Result<(), KeystoreError> {
    match cmd {
        KeystoreCommand::CreateFromMnemonic { mnemonic, generate, name, output_dir } => {
            let dir = match output_dir {
                Some(path) => path.clone(),
                None => std::env::current_dir()?,
            };
            create_from_mnemonic(mnemonic, *generate, name, ProjectLocation::new(dir), None)?;
        }
        KeystoreCommand::CreateFromPrivateKey { private_key, generate, name, output_dir } => {
            let dir = match output_dir {
                Some(path) => path.clone(),
                None => std::env::current_dir()?,
            };
            create_from_private_key(private_key, *generate, name, ProjectLocation::new(dir), None)?;
        }
        KeystoreCommand::Decrypt { path } => {
            decrypt(path)?;
        }
    }

    Ok(())
}

/// Creates a keystore from a mnemonic phrase.
///
/// Either uses the provided mnemonic phrase or generates a new one if `generate` is true.
/// The keystore is encrypted with a password and stored in the project's keystore directory.
///
/// # Arguments
/// * `mnemonic` - Optional existing mnemonic phrase to use
/// * `generate` - Whether to generate a new random mnemonic phrase
/// * `name` - The name for the keystore
/// * `project_location` - The project location for storing the keystore
/// * `password` - Optional password, will prompt user if not provided
///
/// # Returns
/// * `Ok(PathBuf)` - Path to the created keystore file
/// * `Err(KeystoreError)` - Invalid mnemonic, keystore already exists, or creation failed
pub fn create_from_mnemonic(
    mnemonic: &Option<String>,
    generate: bool,
    name: &str,
    project_location: ProjectLocation,
    password: Option<String>,
) -> Result<PathBuf, KeystoreError> {
    project_location.create_keystore_dir()?;
    if project_location.keystore_already_exists(name) {
        return Err(KeystoreError::AlreadyExists(name.to_string()));
    }

    if let Some(phrase) = mnemonic {
        // Throws if the seed phrase is invalid
        let _ = MnemonicBuilder::<English>::default()
            .phrase(phrase)
            .build()
            .map_err(|_| KeystoreError::InvalidMnemonic)?;
    } else if generate {
        // do nothing
    } else {
        return Err(KeystoreError::InvalidMnemonic);
    };

    let password = if password.is_some() {
        password.unwrap()
    } else {
        Password::new()
            .with_prompt("Enter password to encrypt keystore")
            .with_confirmation("Confirm password", "Passwords don't match")
            .interact()?
    };

    if let Some(phrase) = mnemonic {
        store_mnemonic_in_keystore(&phrase, &password, &project_location.get_keystore_dir(), name)?;
    } else {
        create_new_mnemonic_in_keystore(&password, &project_location.get_keystore_dir(), name)?;
    };

    let password_manager = KeyStorePasswordManager::new(&project_location.get_project_name())?;
    password_manager.save(name, &password)?;

    let file_location = project_location.get_keystore_dir().join(name);

    println!("\n✅ Successfully created keystore - {:?}", file_location);
    println!("Account: {}", name);

    Ok(file_location)
}

/// Creates a keystore from a private key.
///
/// Either uses the provided private key or generates a new one if `generate` is true.
/// The keystore is encrypted with a password and stored in the project's account keystore directory.
///
/// # Arguments
/// * `private_key` - Optional existing private key (hex string with or without 0x prefix)
/// * `generate` - Whether to generate a new random private key
/// * `name` - The name for the account keystore
/// * `project_location` - The project location for storing the keystore
/// * `password` - Optional password, will prompt user if not provided
///
/// # Returns
/// * `Ok(PathBuf)` - Path to the created keystore file
/// * `Err(KeystoreError)` - Invalid private key, account already exists, or creation failed
pub fn create_from_private_key(
    private_key: &Option<String>,
    generate: bool,
    name: &str,
    project_location: ProjectLocation,
    password: Option<String>,
) -> Result<PathBuf, KeystoreError> {
    project_location.create_account_keystore_dir()?;

    if project_location.account_already_exists(name) {
        return Err(KeystoreError::AlreadyExists(name.to_string()));
    }

    if let Some(pk) = private_key {
        let pk_str = pk.trim().trim_start_matches("0x");
        let bytes = hex::decode(pk_str)?;

        if bytes.len() != 32 {
            return Err(KeystoreError::InvalidPrivateKey);
        }
    } else if generate {
        // do nothing
    } else {
        return Err(KeystoreError::InvalidPrivateKey);
    };

    let password = if password.is_some() {
        password.unwrap()
    } else {
        Password::new()
            .with_prompt("Enter password to encrypt account")
            .with_confirmation("Confirm password", "Passwords don't match")
            .interact()?
    };

    if let Some(pk) = private_key {
        let private_key =
            LocalSigner::from_str(&pk).map_err(|_| KeystoreError::InvalidPrivateKey)?;
        store_private_key_in_keystore(
            private_key,
            &password,
            &project_location.get_account_keystore_dir(),
            Some(name),
        )?;
    } else {
        create_new_private_key_in_keystore(
            &password,
            &project_location.get_account_keystore_dir(),
            name,
        )?;
    }

    let password_manager = KeyStorePasswordManager::new(&project_location.get_project_name())?;
    password_manager.save(name, &password)?;

    let file_location = project_location.get_account_keystore_dir().join(name);

    println!("\n✅ Successfully created encrypted account - {:?}", file_location);
    println!("Account: {}", name);

    Ok(file_location)
}

/// Decrypts a keystore file and displays its contents.
///
/// Prompts the user for a password and attempts to decrypt the keystore.
/// Allows up to 3 password attempts before failing. Displays the decrypted
/// mnemonic phrase or private key along with the associated address.
///
/// # Arguments
/// * `path` - Path to the keystore file to decrypt
///
/// # Returns
/// * `Ok(())` - Keystore decrypted successfully
/// * `Err(KeystoreError)` - File not found, wrong password, or decryption failed
fn decrypt(path: &PathBuf) -> Result<(), KeystoreError> {
    if !path.exists() || !path.is_file() {
        return Err(KeystoreError::NotFound(format!("{:?}", path)));
    }

    const MAX_ATTEMPTS: usize = 3;
    let mut attempts = 0;

    loop {
        attempts += 1;

        let prompt = if attempts > 1 {
            format!("Wrong password. Try again ({}/{})", attempts, MAX_ATTEMPTS)
        } else {
            "Enter password to decrypt keystore".to_string()
        };

        let password = Password::new().with_prompt(&prompt).interact()?;

        match decrypt_keystore(path, &password) {
            Ok(result) => {
                match result {
                    KeystoreDecryptResult::Mnemonic { phrase, address } => {
                        println!("\n✅ Successfully decrypted mnemonic keystore!");
                        println!("Address: {}", address);

                        println!("\n🔐 Mnemonic Phrase: 🔐");
                        println!("{}", phrase);
                    }
                    KeystoreDecryptResult::PrivateKey { hex_key, address, .. } => {
                        println!("\n✅ Successfully decrypted private key keystore!");
                        println!("Address: {}", address);

                        println!("\n🔐 Private Key: 🔐");
                        println!("{}", hex_key);
                    }
                }
                return Ok(());
            }
            Err(e) => {
                let error_str = e.to_string().to_lowercase();
                let is_likely_password_error = error_str.contains("password")
                    || error_str.contains("mac mismatch")
                    || error_str.contains("invalid")
                    || error_str.contains("decrypt");

                if is_likely_password_error && attempts < MAX_ATTEMPTS {
                    println!("Incorrect password. Try again...");
                    continue;
                } else {
                    return if is_likely_password_error {
                        Err(KeystoreError::DecryptionFailed(format!(
                            "Failed to decrypt after {} attempts. Incorrect password.",
                            MAX_ATTEMPTS
                        )))
                    } else {
                        Err(KeystoreError::DecryptionFailed(format!("Decryption failed: {}", e)))
                    };
                }
            }
        }
    }
}
