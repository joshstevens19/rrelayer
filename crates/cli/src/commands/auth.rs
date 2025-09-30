// use crate::credentials::{self, StoredCredentials};
use crate::credentials::{self};
use crate::error::CliError;
use clap::Subcommand;
// use dialoguer::{Input, Password};
use rand::Rng;
use rand::distributions::Alphanumeric;

#[derive(Subcommand)]
pub enum AuthCommand {
    /// Show authentication status
    Status,
    /// Login and store credentials securely
    // Login {
    //     /// Profile name (defaults to 'default')
    //     #[arg(long, short = 'p')]
    //     profile: Option<String>,
    // },
    /// Logout and remove stored credentials
    // Logout {
    //     /// Profile name (defaults to 'default')
    //     #[arg(long, short = 'p')]
    //     profile: Option<String>,
    // },
    /// List all stored profiles
    List,
    /// Generate an API key which you can then use in the network config
    /// This will not add it to your project for you
    GenApiKey,
}

pub async fn handle_auth_command(cmd: &AuthCommand) -> Result<(), CliError> {
    match cmd {
        AuthCommand::Status => {
            status().await;
        }
        // AuthCommand::Login { profile } => {
        //     let profile_name = profile.as_deref().unwrap_or("default");
        //     login(profile_name).await?;
        // }
        // AuthCommand::Logout { profile } => {
        //     let profile_name = profile.as_deref().unwrap_or("default");
        //     logout(profile_name).await?;
        // }
        AuthCommand::List => {
            list_profiles().await?;
        }
        AuthCommand::GenApiKey => {
            let key = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(8)
                .map(char::from)
                .collect::<String>();
            println!(
                "API key generated - note you have to config it in the networks yaml - {}",
                key
            );
        }
    }

    Ok(())
}

async fn status() -> () {
    use std::env;

    println!("🔐 Authentication Status");
    println!("========================");

    // Check environment variables first
    println!("\n📄 Environment Variables:");
    match env::var("RRELAYER_AUTH_USERNAME") {
        Ok(username) => println!("  ✅ Username: {}", username),
        Err(_) => println!("  ❌ Username: Not configured (RRELAYER_AUTH_USERNAME)"),
    }

    match env::var("RRELAYER_AUTH_PASSWORD") {
        Ok(_) => println!("  ✅ Password: Configured"),
        Err(_) => println!("  ❌ Password: Not configured (RRELAYER_AUTH_PASSWORD)"),
    }

    // Check stored credentials
    println!("\n🔑 Stored Credentials:");

    // First try to load default profile directly
    match credentials::load_credentials("default") {
        Ok(creds) => {
            println!("  ✅ Profile 'default': {} ({})", creds.username, creds.api_url);
        }
        Err(e) => {
            println!("  ❌ Default profile error: {}", e);
        }
    }

    // Also try the profiles list approach
    match credentials::list_profiles() {
        Ok(profiles) if !profiles.is_empty() => {
            println!("  📋 Found profiles: {:?}", profiles);
            for profile in profiles {
                if profile != "default" {
                    // Avoid duplicate output
                    match credentials::load_credentials(&profile) {
                        Ok(creds) => {
                            println!(
                                "  ✅ Profile '{}': {} ({})",
                                profile, creds.username, creds.api_url
                            );
                        }
                        Err(e) => {
                            println!("  ❌ Profile '{}': {}", profile, e);
                        }
                    }
                }
            }
        }
        Ok(_) => println!("  📋 No profiles in list"),
        Err(e) => println!("  📋 Failed to check profile list: {}", e),
    }

    println!("\n💡 Note: Environment variables take precedence over stored credentials.");
    println!("   Use 'rrelayer auth login' to store credentials securely.");
}

// async fn login(profile_name: &str) -> Result<(), CliError> {
//     println!("🔐 Login to RRelayer");
//     println!("===================");
//
//     // Get API URL
//     let api_url: String = Input::new()
//         .with_prompt("API URL")
//         .default("http://localhost:3000".to_string())
//         .interact_text()
//         .map_err(|e| CliError::Input(format!("Failed to get API URL: {}", e)))?;
//
//     // Get username
//     let username: String = Input::new()
//         .with_prompt("Username")
//         .interact_text()
//         .map_err(|e| CliError::Input(format!("Failed to get username: {}", e)))?;
//
//     // Get password
//     let password: String = Password::new()
//         .with_prompt("Password")
//         .interact()
//         .map_err(|e| CliError::Input(format!("Failed to get password: {}", e)))?;
//
//     // Test the credentials
//     println!("\n🧪 Testing credentials...");
//     // let sdk = SDK::new(api_url.clone(), username.clone(), password.clone());
//     //
//     // match sdk.health.check().await {
//     //     Ok(_) => {
//     //         println!("✅ API server is reachable");
//     //     }
//     //     Err(e) => {
//     //         println!("❌ Failed to reach API server: {}", e);
//     //         return Err(CliError::Authentication(
//     //             "API server is not reachable. Please check the URL and try again.".to_string(),
//     //         ));
//     //     }
//     // }
//     //
//     // match sdk.test_auth().await {
//     //     Ok(_) => {
//     //         println!("✅ Authentication successful");
//     //     }
//     //     Err(e) => {
//     //         println!("❌ Authentication failed: {}", e);
//     //         return Err(CliError::Authentication(
//     //             "Invalid credentials. Please check your username and password.".to_string(),
//     //         ));
//     //     }
//     // }
//
//     // Store credentials
//     let credentials = StoredCredentials { api_url, username, password };
//
//     // Store credentials with detailed error reporting
//     match credentials::store_credentials(profile_name, &credentials) {
//         Ok(_) => println!("🔒 Credentials storage: Success"),
//         Err(e) => {
//             println!("🔒 Credentials storage: Failed - {}", e);
//             return Err(CliError::Storage(format!("Failed to store credentials: {}", e)));
//         }
//     }
//
//     // Update profile list with detailed error reporting
//     match credentials::add_profile_to_list(profile_name) {
//         Ok(_) => println!("📋 Profile list update: Success"),
//         Err(e) => {
//             println!("📋 Profile list update: Failed - {}", e);
//             return Err(CliError::Storage(format!("Failed to update profile list: {}", e)));
//         }
//     }
//
//     // Immediately test retrieval
//     println!("🧪 Testing immediate retrieval...");
//     match credentials::load_credentials(profile_name) {
//         Ok(test_creds) => {
//             println!(
//                 "✅ Immediate retrieval successful: {} at {}",
//                 test_creds.username, test_creds.api_url
//             );
//         }
//         Err(e) => {
//             println!("❌ Immediate retrieval failed: {}", e);
//         }
//     }
//
//     println!("✅ Credentials stored successfully for profile '{}'", profile_name);
//     println!("💡 You can now use RRelayer CLI without environment variables");
//
//     Ok(())
// }

// async fn logout(profile_name: &str) -> Result<(), CliError> {
//     println!("🚪 Logout from RRelayer");
//     println!("=====================");
//
//     match credentials::load_credentials(profile_name) {
//         Ok(_) => {
//             credentials::delete_credentials(profile_name)
//                 .map_err(|e| CliError::Storage(format!("Failed to delete credentials: {}", e)))?;
//
//             credentials::remove_profile_from_list(profile_name)
//                 .map_err(|e| CliError::Storage(format!("Failed to update profile list: {}", e)))?;
//
//             println!("✅ Successfully logged out profile '{}'", profile_name);
//         }
//         Err(credentials::CredentialError::NotFound) => {
//             println!("❌ Profile '{}' not found", profile_name);
//             return Err(CliError::NotFound(format!("Profile '{}' not found", profile_name)));
//         }
//         Err(e) => {
//             return Err(CliError::Storage(format!("Failed to load credentials: {}", e)));
//         }
//     }
//
//     Ok(())
// }

async fn list_profiles() -> Result<(), CliError> {
    println!("📋 Stored Profiles");
    println!("=================");

    match credentials::list_profiles() {
        Ok(profiles) if profiles.is_empty() => {
            println!("❌ No stored profiles found");
            println!("💡 Use 'rrelayer auth login' to create a profile");
        }
        Ok(profiles) => {
            for profile in profiles {
                match credentials::load_credentials(&profile) {
                    Ok(creds) => {
                        println!(
                            "✅ Profile '{}': {} ({})",
                            profile, creds.username, creds.api_url
                        );
                    }
                    Err(_) => {
                        println!("❌ Profile '{}': Failed to load", profile);
                    }
                }
            }
        }
        Err(e) => {
            return Err(CliError::Storage(format!("Failed to list profiles: {}", e)));
        }
    }

    Ok(())
}
