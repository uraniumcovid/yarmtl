use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoistTask {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<TodoistDue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_completed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoistDue {
    pub date: String, // YYYY-MM-DD format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datetime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoistLabel {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone)]
pub struct YarmtlMetadata {
    pub id: String,
    pub reminder: Option<String>, // Date string YYYY-MM-DD
    pub notes: Option<String>,
    pub importance: Option<u8>,
}

impl YarmtlMetadata {
    pub fn encode(&self) -> String {
        let mut meta = String::from("[YARMTL-META]\n");
        meta.push_str(&format!("id: {}\n", self.id));

        if let Some(reminder) = &self.reminder {
            meta.push_str(&format!("reminder: {}\n", reminder));
        }

        if let Some(importance) = self.importance {
            meta.push_str(&format!("importance: {}\n", importance));
        }

        if let Some(notes) = &self.notes {
            meta.push_str(&format!("notes: {}\n", notes));
        }

        meta.push_str("[/YARMTL-META]");
        meta
    }

    pub fn parse(description: &str) -> Option<Self> {
        if !description.contains("[YARMTL-META]") || !description.contains("[/YARMTL-META]") {
            return None;
        }

        let start = description.find("[YARMTL-META]")? + "[YARMTL-META]".len();
        let end = description.find("[/YARMTL-META]")?;
        let meta_section = &description[start..end];

        let mut id = None;
        let mut reminder = None;
        let mut notes = None;
        let mut importance = None;

        for line in meta_section.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "id" => id = Some(value.to_string()),
                    "reminder" => reminder = Some(value.to_string()),
                    "importance" => importance = value.parse().ok(),
                    "notes" => notes = Some(value.to_string()),
                    _ => {}
                }
            }
        }

        Some(YarmtlMetadata {
            id: id?,
            reminder,
            notes,
            importance,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_encode_decode() {
        let meta = YarmtlMetadata {
            id: "abc12345".to_string(),
            reminder: Some("2026-01-28".to_string()),
            notes: Some("Important task".to_string()),
            importance: Some(3),
        };

        let encoded = meta.encode();
        let decoded = YarmtlMetadata::parse(&encoded).unwrap();

        assert_eq!(decoded.id, "abc12345");
        assert_eq!(decoded.reminder, Some("2026-01-28".to_string()));
        assert_eq!(decoded.notes, Some("Important task".to_string()));
        assert_eq!(decoded.importance, Some(3));
    }

    #[test]
    fn test_metadata_parse_none() {
        let description = "Regular task description without metadata";
        assert!(YarmtlMetadata::parse(description).is_none());
    }
}
