use keyring::Entry;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::PathBuf;

const KEYRING_SERVICE: &str = "yarmtl-todoist";
const KEYRING_USERNAME: &str = "api-token";

#[derive(Debug)]
pub enum AuthError {
    KeyringError(String),
    TokenNotFound,
    InvalidToken,
    IoError(String),
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::KeyringError(msg) => write!(f, "Keyring error: {}", msg),
            AuthError::TokenNotFound => write!(f, "Todoist API token not found. Run 'yarmtl --setup-todoist' to configure."),
            AuthError::InvalidToken => write!(f, "Invalid Todoist API token"),
            AuthError::IoError(msg) => write!(f, "IO error: {}", msg),
        }
    }
}

impl Error for AuthError {}

pub struct TodoistAuth;

impl TodoistAuth {
    fn get_token_file_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home)
            .join(".local/share/yarmtl")
            .join(".todoist_token")
    }

    pub fn store_token(token: &str) -> Result<(), AuthError> {
        // Try keyring first
        match Entry::new(KEYRING_SERVICE, KEYRING_USERNAME) {
            Ok(entry) => {
                if let Ok(()) = entry.set_password(token) {
                    return Ok(());
                }
            }
            Err(_) => {}
        }

        // Fallback to file storage
        eprintln!("⚠ System keyring not available, using file storage (less secure)");
        let token_file = Self::get_token_file_path();

        // Create parent directory if needed
        if let Some(parent) = token_file.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| AuthError::IoError(e.to_string()))?;
        }

        // Write token to file with restricted permissions
        fs::write(&token_file, token)
            .map_err(|e| AuthError::IoError(e.to_string()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&token_file, fs::Permissions::from_mode(0o600))
                .map_err(|e| AuthError::IoError(e.to_string()))?;
        }

        Ok(())
    }

    pub fn get_token() -> Result<String, AuthError> {
        // Try keyring first
        match Entry::new(KEYRING_SERVICE, KEYRING_USERNAME) {
            Ok(entry) => {
                if let Ok(token) = entry.get_password() {
                    return Ok(token);
                }
            }
            Err(_) => {}
        }

        // Fallback to file storage
        let token_file = Self::get_token_file_path();
        if !token_file.exists() {
            return Err(AuthError::TokenNotFound);
        }

        fs::read_to_string(&token_file)
            .map(|s| s.trim().to_string())
            .map_err(|_| AuthError::TokenNotFound)
    }

    pub fn delete_token() -> Result<(), AuthError> {
        // Try keyring first
        match Entry::new(KEYRING_SERVICE, KEYRING_USERNAME) {
            Ok(entry) => {
                let _ = entry.delete_password();
            }
            Err(_) => {}
        }

        // Also delete file if exists
        let token_file = Self::get_token_file_path();
        if token_file.exists() {
            fs::remove_file(&token_file)
                .map_err(|e| AuthError::IoError(e.to_string()))?;
        }

        Ok(())
    }

    pub async fn verify_token(token: &str) -> Result<bool, Box<dyn Error>> {
        let client = reqwest::Client::new();
        let response = client
            .get("https://api.todoist.com/rest/v2/projects")
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;

        Ok(response.status().is_success())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires system keyring access, not available in Nix sandbox
    fn test_token_operations() {
        let test_token = "test-token-12345";

        // Clean up any existing token
        let _ = TodoistAuth::delete_token();

        // Store token
        assert!(TodoistAuth::store_token(test_token).is_ok());

        // Retrieve token
        let retrieved = TodoistAuth::get_token().unwrap();
        assert_eq!(retrieved, test_token);

        // Delete token
        assert!(TodoistAuth::delete_token().is_ok());

        // Verify token is deleted
        assert!(TodoistAuth::get_token().is_err());
    }
}
