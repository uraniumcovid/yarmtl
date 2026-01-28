use regex::Regex;
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
    pub due_date: Option<String>, // For sending to API: YYYY-MM-DD
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoistProject {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[derive(Debug, Clone)]
pub struct YarmtlMetadata {
    pub id: String,
    pub deadline: Option<String>, // Date string YYYY-MM-DD
    pub reminder: Option<String>, // Date string YYYY-MM-DD
    pub notes: Option<String>,
    pub importance: Option<u8>,
}

impl YarmtlMetadata {
    pub fn encode(&self) -> String {
        let mut meta = String::new();

        // Add deadline using !date syntax
        if let Some(deadline) = &self.deadline {
            meta.push_str(&format!("!{} ", deadline));
        }

        // Add reminder using @date syntax
        if let Some(reminder) = &self.reminder {
            meta.push_str(&format!("@{} ", reminder));
        }

        // Add importance using $1-5 syntax
        if let Some(importance) = self.importance {
            meta.push_str(&format!("${} ", importance));
        }

        // Add notes using //notes syntax
        if let Some(notes) = &self.notes {
            meta.push_str(&format!("//{} ", notes));
        }

        // Add yarmtl ID at the end
        meta.push_str(&format!("[yarmtl:{}]", self.id));

        meta.trim().to_string()
    }

    pub fn parse(description: &str) -> Option<Self> {
        // Extract yarmtl ID - if not present, this isn't a yarmtl task
        let id_re = Regex::new(r"\[yarmtl:([a-f0-9-]+)\]").ok()?;
        let id = id_re.captures(description)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().to_string())?;

        // Extract deadline (!date)
        let deadline_re = Regex::new(r"!(\d{4}-\d{2}-\d{2})").ok()?;
        let deadline = deadline_re.captures(description)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().to_string());

        // Extract reminder (@date)
        let reminder_re = Regex::new(r"@(\d{4}-\d{2}-\d{2})").ok()?;
        let reminder = reminder_re.captures(description)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().to_string());

        // Extract importance ($1-5)
        let importance_re = Regex::new(r"\$([1-5])").ok()?;
        let importance = importance_re.captures(description)
            .and_then(|cap| cap.get(1))
            .and_then(|m| m.as_str().parse().ok());

        // Extract notes (//text)
        let notes_re = Regex::new(r"//([^$@!\[]+)").ok()?;
        let notes = notes_re.captures(description)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().trim().to_string());

        Some(YarmtlMetadata {
            id,
            deadline,
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
            deadline: Some("2026-01-30".to_string()),
            reminder: Some("2026-01-28".to_string()),
            notes: Some("Important task".to_string()),
            importance: Some(3),
        };

        let encoded = meta.encode();
        // Should be in format: !2026-01-30 @2026-01-28 $3 //Important task [yarmtl:abc12345]
        assert!(encoded.contains("!2026-01-30"));
        assert!(encoded.contains("@2026-01-28"));
        assert!(encoded.contains("$3"));
        assert!(encoded.contains("//Important task"));
        assert!(encoded.contains("[yarmtl:abc12345]"));

        let decoded = YarmtlMetadata::parse(&encoded).unwrap();

        assert_eq!(decoded.id, "abc12345");
        assert_eq!(decoded.deadline, Some("2026-01-30".to_string()));
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
