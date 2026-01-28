use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SyncMetadata {
    pub last_sync: DateTime<Utc>,
    pub task_mappings: HashMap<String, TaskSyncInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TaskSyncInfo {
    pub todoist_id: String,
    pub last_modified: DateTime<Utc>,
    pub last_sync_hash: String,
}

impl SyncMetadata {
    pub fn new() -> Self {
        SyncMetadata {
            last_sync: Utc::now(),
            task_mappings: HashMap::new(),
        }
    }

    pub fn load(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        if !path.exists() {
            return Ok(Self::new());
        }

        let content = fs::read_to_string(path)?;
        let metadata: SyncMetadata = serde_json::from_str(&content)?;
        Ok(metadata)
    }

    pub fn save(&self, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn get_todoist_id(&self, yarmtl_id: &str) -> Option<&str> {
        self.task_mappings
            .get(yarmtl_id)
            .map(|info| info.todoist_id.as_str())
    }

    pub fn get_yarmtl_id(&self, todoist_id: &str) -> Option<String> {
        self.task_mappings
            .iter()
            .find(|(_, info)| info.todoist_id == todoist_id)
            .map(|(yarmtl_id, _)| yarmtl_id.clone())
    }

    pub fn update_mapping(&mut self, yarmtl_id: String, info: TaskSyncInfo) {
        self.task_mappings.insert(yarmtl_id, info);
    }

    pub fn remove_mapping(&mut self, yarmtl_id: &str) {
        self.task_mappings.remove(yarmtl_id);
    }

    pub fn update_last_sync(&mut self) {
        self.last_sync = Utc::now();
    }

    pub fn get_hash(&self, yarmtl_id: &str) -> Option<&str> {
        self.task_mappings
            .get(yarmtl_id)
            .map(|info| info.last_sync_hash.as_str())
    }
}

impl Default for SyncMetadata {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_metadata() {
        let metadata = SyncMetadata::new();
        assert!(metadata.task_mappings.is_empty());
    }

    #[test]
    fn test_add_and_retrieve_mapping() {
        let mut metadata = SyncMetadata::new();
        let info = TaskSyncInfo {
            todoist_id: "todoist123".to_string(),
            last_modified: Utc::now(),
            last_sync_hash: "hash123".to_string(),
        };

        metadata.update_mapping("yarmtl123".to_string(), info);

        assert_eq!(
            metadata.get_todoist_id("yarmtl123"),
            Some("todoist123")
        );
        assert_eq!(
            metadata.get_yarmtl_id("todoist123"),
            Some("yarmtl123".to_string())
        );
    }

    #[test]
    fn test_remove_mapping() {
        let mut metadata = SyncMetadata::new();
        let info = TaskSyncInfo {
            todoist_id: "todoist123".to_string(),
            last_modified: Utc::now(),
            last_sync_hash: "hash123".to_string(),
        };

        metadata.update_mapping("yarmtl123".to_string(), info);
        metadata.remove_mapping("yarmtl123");

        assert_eq!(metadata.get_todoist_id("yarmtl123"), None);
    }
}
