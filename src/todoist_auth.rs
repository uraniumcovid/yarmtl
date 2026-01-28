use keyring::Entry;
use std::error::Error;
use std::fmt;

const KEYRING_SERVICE: &str = "yarmtl-todoist";
const KEYRING_USERNAME: &str = "api-token";

#[derive(Debug)]
pub enum AuthError {
    KeyringError(String),
    TokenNotFound,
    InvalidToken,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::KeyringError(msg) => write!(f, "Keyring error: {}", msg),
            AuthError::TokenNotFound => write!(f, "Todoist API token not found. Run 'yarmtl --setup-todoist' to configure."),
            AuthError::InvalidToken => write!(f, "Invalid Todoist API token"),
        }
    }
}

impl Error for AuthError {}

pub struct TodoistAuth;

impl TodoistAuth {
    pub fn store_token(token: &str) -> Result<(), AuthError> {
        let entry = Entry::new(KEYRING_SERVICE, KEYRING_USERNAME)
            .map_err(|e| AuthError::KeyringError(e.to_string()))?;

        entry
            .set_password(token)
            .map_err(|e| AuthError::KeyringError(e.to_string()))?;

        Ok(())
    }

    pub fn get_token() -> Result<String, AuthError> {
        let entry = Entry::new(KEYRING_SERVICE, KEYRING_USERNAME)
            .map_err(|e| AuthError::KeyringError(e.to_string()))?;

        entry
            .get_password()
            .map_err(|_| AuthError::TokenNotFound)
    }

    pub fn delete_token() -> Result<(), AuthError> {
        let entry = Entry::new(KEYRING_SERVICE, KEYRING_USERNAME)
            .map_err(|e| AuthError::KeyringError(e.to_string()))?;

        entry
            .delete_password()
            .map_err(|e| AuthError::KeyringError(e.to_string()))?;

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
