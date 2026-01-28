use crate::todoist_types::{TodoistTask, TodoistLabel, TodoistProject};
use reqwest::Client;
use serde_json::json;
use std::time::Duration;
use thiserror::Error;

const API_BASE_URL: &str = "https://api.todoist.com/rest/v2";

#[derive(Error, Debug)]
pub enum TodoistError {
    #[error("Authentication failed: {0}")]
    AuthError(String),

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("Rate limit exceeded, retry after {retry_after} seconds")]
    RateLimitExceeded { retry_after: u64 },

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("API error: {status} - {message}")]
    ApiError { status: u16, message: String },

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

pub struct TodoistClient {
    client: Client,
    api_token: String,
}

impl TodoistClient {
    pub fn new(api_token: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        TodoistClient { client, api_token }
    }

    async fn make_request<T: serde::de::DeserializeOwned>(
        &self,
        method: reqwest::Method,
        endpoint: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T, TodoistError> {
        let url = format!("{}{}", API_BASE_URL, endpoint);

        let mut request = self
            .client
            .request(method, &url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json");

        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request.send().await?;

        let status = response.status();

        if status.is_success() {
            let result = response.json::<T>().await?;
            Ok(result)
        } else if status.as_u16() == 429 {
            let retry_after = response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse().ok())
                .unwrap_or(60);

            Err(TodoistError::RateLimitExceeded { retry_after })
        } else if status.as_u16() == 401 {
            Err(TodoistError::AuthError(
                "Invalid API token".to_string(),
            ))
        } else if status.as_u16() == 404 {
            Err(TodoistError::TaskNotFound(endpoint.to_string()))
        } else {
            let error_text = response.text().await.unwrap_or_default();
            Err(TodoistError::ApiError {
                status: status.as_u16(),
                message: error_text,
            })
        }
    }

    pub async fn list_tasks(&self) -> Result<Vec<TodoistTask>, TodoistError> {
        // Only fetch active (non-completed) tasks to avoid syncing thousands of old tasks
        let url = "/tasks";
        self.make_request(reqwest::Method::GET, url, None)
            .await
    }

    pub async fn get_task(&self, task_id: &str) -> Result<TodoistTask, TodoistError> {
        let endpoint = format!("/tasks/{}", task_id);
        self.make_request(reqwest::Method::GET, &endpoint, None)
            .await
    }

    pub async fn create_task(&self, task: &TodoistTask) -> Result<TodoistTask, TodoistError> {
        let body = serde_json::to_value(task)?;
        self.make_request(reqwest::Method::POST, "/tasks", Some(body))
            .await
    }

    pub async fn update_task(
        &self,
        task_id: &str,
        task: &TodoistTask,
    ) -> Result<TodoistTask, TodoistError> {
        let endpoint = format!("/tasks/{}", task_id);
        let body = serde_json::to_value(task)?;
        self.make_request(reqwest::Method::POST, &endpoint, Some(body))
            .await
    }

    pub async fn delete_task(&self, task_id: &str) -> Result<(), TodoistError> {
        let endpoint = format!("/tasks/{}", task_id);
        self.make_request::<serde_json::Value>(reqwest::Method::DELETE, &endpoint, None)
            .await?;
        Ok(())
    }

    pub async fn close_task(&self, task_id: &str) -> Result<(), TodoistError> {
        let endpoint = format!("/tasks/{}/close", task_id);
        self.make_request::<serde_json::Value>(reqwest::Method::POST, &endpoint, None)
            .await?;
        Ok(())
    }

    pub async fn reopen_task(&self, task_id: &str) -> Result<(), TodoistError> {
        let endpoint = format!("/tasks/{}/reopen", task_id);
        self.make_request::<serde_json::Value>(reqwest::Method::POST, &endpoint, None)
            .await?;
        Ok(())
    }

    pub async fn list_labels(&self) -> Result<Vec<TodoistLabel>, TodoistError> {
        self.make_request(reqwest::Method::GET, "/labels", None)
            .await
    }

    pub async fn create_label(&self, name: &str) -> Result<TodoistLabel, TodoistError> {
        let body = json!({
            "name": name
        });
        self.make_request(reqwest::Method::POST, "/labels", Some(body))
            .await
    }

    pub async fn list_projects(&self) -> Result<Vec<TodoistProject>, TodoistError> {
        self.make_request(reqwest::Method::GET, "/projects", None)
            .await
    }

    pub async fn create_project(&self, name: &str) -> Result<TodoistProject, TodoistError> {
        let body = json!({
            "name": name
        });
        self.make_request(reqwest::Method::POST, "/projects", Some(body))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = TodoistClient::new("test-token".to_string());
        assert_eq!(client.api_token, "test-token");
    }
}
