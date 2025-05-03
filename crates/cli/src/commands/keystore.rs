use std::{fs, path::PathBuf, str::FromStr};

use alloy::signers::local::{LocalSigner, MnemonicBuilder, coins_bip39::English};
use clap::Subcommand;
use dialoguer::Password;
use rrelayerr_core::{
    SetupConfig,
    keystore::{
        KeyStorePasswordManager, KeystoreDecryptResult, create_new_mnemonic_in_keystore,
        create_new_private_key_in_keystore, decrypt_keystore, store_mnemonic_in_keystore,
        store_private_key_in_keystore,
    },
    read,
};

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
    pub fn new(output_dir: PathBuf) -> Self {
        Self { output_dir, override_project_name: None }
    }

    pub fn override_project_name(&mut self, name: &str) {
        self.override_project_name = Some(name.to_string());
    }

    fn get_keystore_dir(&self) -> PathBuf {
        self.output_dir.join("keystores")
    }

    fn keystore_already_exists(&self, name: &str) -> bool {
        self.get_keystore_dir().join(name).exists()
    }

    fn get_account_keystore_dir(&self) -> PathBuf {
        self.output_dir.join("keystores").join("accounts")
    }

    pub fn get_account_keystore(&self, account: &str) -> PathBuf {
        self.output_dir.join("keystores").join("accounts").join(account)
    }

    fn create_keystore_dir(&self) -> Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all(&self.get_keystore_dir())?;
        Ok(())
    }

    fn create_account_keystore_dir(&self) -> Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all(&self.get_account_keystore_dir())?;
        Ok(())
    }

    fn account_already_exists(&self, name: &str) -> bool {
        self.get_account_keystore_dir().join(name).exists()
    }

    pub fn setup_config(&self) -> Result<SetupConfig, Box<dyn std::error::Error>> {
        let yaml = read(&self.output_dir.join("rrelayerr.yaml"))?;
        Ok(yaml)
    }

    pub fn get_project_name(&self) -> String {
        self.override_project_name
            .clone()
            .unwrap_or_else(|| self.setup_config().unwrap().name.clone())
    }

    pub fn get_api_url(&self) -> Result<String, Box<dyn std::error::Error>> {
        let setup_config = self.setup_config()?;
        Ok(format!("http://localhost:{}", setup_config.api_config.port))
    }
}

pub async fn handle_keystore_command(
    cmd: &KeystoreCommand,
) -> Result<(), Box<dyn std::error::Error>> {
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

pub fn create_from_mnemonic(
    mnemonic: &Option<String>,
    generate: bool,
    name: &str,
    project_location: ProjectLocation,
    password: Option<String>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    project_location.create_keystore_dir()?;
    if project_location.keystore_already_exists(name) {
        return Err(format!("Keystore already exists: {}", name).into());
    }

    if let Some(phrase) = mnemonic {
        // Throws if the seed phrase is invalid
        let _ = MnemonicBuilder::<English>::default().phrase(phrase).build()?;
    } else if generate {
        // do nothing
    } else {
        return Err("Either --mnemonic or --generate must be specified".into());
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

    let password_manager = KeyStorePasswordManager::new(&project_location.get_project_name());
    password_manager.save(name, &password)?;

    let file_location = project_location.get_keystore_dir().join(name);

    println!("\n✅ Successfully created keystore - {:?}", file_location);
    println!("Account: {}", name);

    Ok(file_location)
}

pub fn create_from_private_key(
    private_key: &Option<String>,
    generate: bool,
    name: &str,
    project_location: ProjectLocation,
    password: Option<String>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    project_location.create_account_keystore_dir()?;

    if project_location.account_already_exists(name) {
        return Err(format!("Account already exists: {}", name).into());
    }

    if let Some(pk) = private_key {
        let pk_str = pk.trim().trim_start_matches("0x");
        let bytes = hex::decode(pk_str)?;

        if bytes.len() != 32 {
            return Err(format!("Invalid private key length: {}, expected 32", bytes.len()).into());
        }
    } else if generate {
        // do nothing
    } else {
        return Err("Either --private-key or --generate must be specified".into());
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
        let private_key = LocalSigner::from_str(&pk)?;
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

    let password_manager = KeyStorePasswordManager::new(&project_location.get_project_name());
    password_manager.save(name, &password)?;

    let file_location = project_location.get_account_keystore_dir().join(name);

    println!("\n✅ Successfully created encrypted account - {:?}", file_location);
    println!("Account: {}", name);

    Ok(file_location)
}

fn decrypt(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    if !path.exists() || !path.is_file() {
        return Err(format!("Keystore file not found: {:?}", path).into());
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
                let is_likely_password_error = error_str.contains("password") ||
                    error_str.contains("mac mismatch") ||
                    error_str.contains("invalid") ||
                    error_str.contains("decrypt");

                if is_likely_password_error && attempts < MAX_ATTEMPTS {
                    println!("Incorrect password. Try again...");
                    continue;
                } else {
                    return if is_likely_password_error {
                        Err(format!(
                            "Failed to decrypt after {} attempts. Incorrect password.",
                            MAX_ATTEMPTS
                        )
                        .into())
                    } else {
                        Err(e)
                    };
                }
            }
        }
    }
}
