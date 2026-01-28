use crate::sync_metadata::{SyncMetadata, TaskSyncInfo};
use crate::todoist_client::TodoistClient;
use crate::todoist_types::{TodoistTask, TodoistDue, YarmtlMetadata};
use chrono::{NaiveDate, Utc};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

// Import Task from main
use crate::Task;

#[derive(Debug)]
pub struct SyncReport {
    pub created_in_todoist: usize,
    pub created_in_yarmtl: usize,
    pub updated_in_todoist: usize,
    pub updated_in_yarmtl: usize,
    pub deleted_in_todoist: usize,
    pub deleted_in_yarmtl: usize,
    pub conflicts_resolved: usize,
}

impl SyncReport {
    pub fn new() -> Self {
        SyncReport {
            created_in_todoist: 0,
            created_in_yarmtl: 0,
            updated_in_todoist: 0,
            updated_in_yarmtl: 0,
            deleted_in_todoist: 0,
            deleted_in_yarmtl: 0,
            conflicts_resolved: 0,
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "↑{} ↓{} ⇅{} ✗{}",
            self.created_in_todoist + self.updated_in_todoist,
            self.created_in_yarmtl + self.updated_in_yarmtl,
            self.conflicts_resolved,
            self.deleted_in_todoist + self.deleted_in_yarmtl
        )
    }
}

#[derive(Debug)]
pub enum SyncAction {
    CreateInTodoist(Task),
    CreateInYarmtl(TodoistTask),
    UpdateTodoist { yarmtl_id: String, task: Task },
    UpdateYarmtl { todoist_id: String, task: TodoistTask },
    DeleteFromTodoist { todoist_id: String },
    DeleteFromYarmtl { yarmtl_id: String },
}

pub struct TodoistSync {
    client: TodoistClient,
    metadata: SyncMetadata,
    metadata_path: PathBuf,
    local_tasks: Vec<Task>,
    tasks_modified: bool,
    projects: HashMap<String, String>, // project_name -> project_id
}

impl TodoistSync {
    pub fn new(api_token: String, sync_dir: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let client = TodoistClient::new(api_token);
        let metadata_path = sync_dir.join(".sync_metadata.json");
        let metadata = SyncMetadata::load(&metadata_path)?;

        Ok(TodoistSync {
            client,
            metadata,
            metadata_path,
            local_tasks: Vec::new(),
            tasks_modified: false,
            projects: HashMap::new(),
        })
    }

    pub async fn sync(&mut self, tasks_file: &PathBuf) -> Result<SyncReport, Box<dyn std::error::Error>> {
        let mut report = SyncReport::new();

        // Fetch all projects from Todoist
        let projects = self.client.list_projects().await?;
        self.projects = projects
            .into_iter()
            .map(|p| (p.name.clone(), p.id.clone()))
            .collect();

        // Fetch all tasks from Todoist
        let todoist_tasks = self.client.list_tasks().await?;

        // Load local tasks
        self.local_tasks = self.load_local_tasks(tasks_file)?;
        self.tasks_modified = false;

        // Detect changes
        let actions = self.detect_changes(&self.local_tasks.clone(), &todoist_tasks);

        // Apply actions
        let total_actions = actions.len();
        for (idx, action) in actions.into_iter().enumerate() {
            eprint!("\r🔄 Syncing... {}/{} ", idx + 1, total_actions);
            match self.apply_action(action).await {
                Ok(action_type) => {
                    match action_type {
                        ActionType::CreatedInTodoist => report.created_in_todoist += 1,
                        ActionType::CreatedInYarmtl => report.created_in_yarmtl += 1,
                        ActionType::UpdatedInTodoist => report.updated_in_todoist += 1,
                        ActionType::UpdatedInYarmtl => report.updated_in_yarmtl += 1,
                        ActionType::DeletedFromTodoist => report.deleted_in_todoist += 1,
                        ActionType::DeletedFromYarmtl => report.deleted_in_yarmtl += 1,
                    }
                }
                Err(e) => {
                    eprintln!("\r⚠ Sync action failed: {}", e);
                }
            }
        }
        if total_actions > 0 {
            eprintln!("\r✓ Completed {} sync actions", total_actions);
        }

        // Write back local tasks if modified
        if self.tasks_modified {
            self.save_local_tasks(tasks_file)?;
        }

        // Update last sync timestamp
        self.metadata.update_last_sync();

        // Save metadata
        self.metadata.save(&self.metadata_path)?;

        Ok(report)
    }

    fn save_local_tasks(&self, tasks_file: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let mut content = String::from("# tasks\n\n");

        for task in &self.local_tasks {
            content.push_str(&format!("{}\n", task.to_markdown()));
        }

        fs::write(tasks_file, content)?;
        Ok(())
    }

    fn load_local_tasks(&self, tasks_file: &PathBuf) -> Result<Vec<Task>, Box<dyn std::error::Error>> {
        if !tasks_file.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(tasks_file)?;
        let mut tasks = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("- [ ]") || trimmed.starts_with("- [x]") {
                let task_text = trimmed
                    .strip_prefix("- [ ] ")
                    .or_else(|| trimmed.strip_prefix("- [x] "))
                    .unwrap_or(trimmed);

                let mut task = Task::parse(task_text);
                task.completed = trimmed.starts_with("- [x]");
                tasks.push(task);
            }
        }

        Ok(tasks)
    }

    fn detect_changes(&self, local_tasks: &[Task], todoist_tasks: &[TodoistTask]) -> Vec<SyncAction> {
        let mut actions = Vec::new();

        // Build sets for quick lookup
        let local_ids: HashSet<_> = local_tasks.iter().map(|t| t.id.clone()).collect();
        let todoist_ids: HashSet<_> = todoist_tasks
            .iter()
            .filter_map(|t| t.id.clone())
            .collect();

        // Map of todoist_id -> task (for future use)
        let _todoist_map: HashMap<_, _> = todoist_tasks
            .iter()
            .filter_map(|t| t.id.as_ref().map(|id| (id.clone(), t)))
            .collect();

        // Check local tasks
        for local_task in local_tasks {
            if let Some(todoist_id) = self.metadata.get_todoist_id(&local_task.id) {
                // Task is mapped
                if todoist_ids.contains(todoist_id) {
                    // Both exist - check for changes
                    let local_hash = self.compute_task_hash(local_task);
                    let stored_hash = self.metadata.get_hash(&local_task.id);

                    if stored_hash.map(|h| h != local_hash).unwrap_or(true) {
                        // Local changed, update Todoist
                        actions.push(SyncAction::UpdateTodoist {
                            yarmtl_id: local_task.id.clone(),
                            task: local_task.clone(),
                        });
                    }
                } else {
                    // Todoist task was deleted
                    actions.push(SyncAction::DeleteFromYarmtl {
                        yarmtl_id: local_task.id.clone(),
                    });
                }
            } else {
                // Task not in metadata - could be new, or old completed task
                // Skip completed tasks (don't sync old completed tasks to Todoist)
                if local_task.completed {
                    // Only sync completed tasks if they have a deadline in the future
                    // or within the last 30 days
                    let should_skip = if let Some(deadline) = local_task.deadline {
                        let today = chrono::Local::now().date_naive();
                        let thirty_days_ago = today - chrono::Duration::days(30);
                        deadline < thirty_days_ago
                    } else {
                        // No deadline - skip all old completed tasks
                        true
                    };

                    if should_skip {
                        continue;
                    }
                }

                // New local task - create in Todoist
                actions.push(SyncAction::CreateInTodoist(local_task.clone()));
            }
        }

        // Check Todoist tasks
        for todoist_task in todoist_tasks {
            if let Some(todoist_id) = &todoist_task.id {
                if let Some(yarmtl_id) = self.metadata.get_yarmtl_id(todoist_id) {
                    // Already mapped, handled above
                    if !local_ids.contains(&yarmtl_id) {
                        // Local was deleted
                        actions.push(SyncAction::DeleteFromTodoist {
                            todoist_id: todoist_id.clone(),
                        });
                    }
                } else {
                    // Check if this is a new Todoist task or has yarmtl metadata
                    if let Some(meta) = self.extract_yarmtl_metadata(todoist_task) {
                        // Has yarmtl metadata, might be an update
                        if local_ids.contains(&meta.id) {
                            // Update local
                            actions.push(SyncAction::UpdateYarmtl {
                                todoist_id: todoist_id.clone(),
                                task: todoist_task.clone(),
                            });
                        } else {
                            // Create new local
                            actions.push(SyncAction::CreateInYarmtl(todoist_task.clone()));
                        }
                    } else {
                        // New Todoist task without metadata
                        actions.push(SyncAction::CreateInYarmtl(todoist_task.clone()));
                    }
                }
            }
        }

        actions
    }

    async fn apply_action(&mut self, action: SyncAction) -> Result<ActionType, Box<dyn std::error::Error>> {
        match action {
            SyncAction::CreateInTodoist(task) => {
                // Ensure project exists if task has tags
                if !task.tags.is_empty() {
                    self.get_or_create_project(&task.tags[0]).await;
                }

                let todoist_task = self.convert_yarmtl_to_todoist(&task);
                let created = self.client.create_task(&todoist_task).await?;

                if let Some(todoist_id) = created.id {
                    // If task is completed, close it in Todoist
                    if task.completed {
                        let _ = self.client.close_task(&todoist_id).await;
                    }

                    let info = TaskSyncInfo {
                        todoist_id: todoist_id.clone(),
                        last_modified: Utc::now(),
                        last_sync_hash: self.compute_task_hash(&task),
                    };
                    self.metadata.update_mapping(task.id, info);
                }

                Ok(ActionType::CreatedInTodoist)
            }
            SyncAction::CreateInYarmtl(todoist_task) => {
                let yarmtl_task = self.convert_todoist_to_yarmtl(&todoist_task);

                if let Some(todoist_id) = todoist_task.id {
                    let info = TaskSyncInfo {
                        todoist_id,
                        last_modified: Utc::now(),
                        last_sync_hash: self.compute_task_hash(&yarmtl_task),
                    };
                    self.metadata.update_mapping(yarmtl_task.id.clone(), info);
                }

                // Add to local tasks
                self.local_tasks.push(yarmtl_task);
                self.tasks_modified = true;

                Ok(ActionType::CreatedInYarmtl)
            }
            SyncAction::UpdateTodoist { yarmtl_id, task } => {
                if let Some(todoist_id) = self.metadata.get_todoist_id(&yarmtl_id).map(|s| s.to_string()) {
                    // Ensure project exists if task has tags
                    if !task.tags.is_empty() {
                        self.get_or_create_project(&task.tags[0]).await;
                    }

                    let todoist_task = self.convert_yarmtl_to_todoist(&task);
                    self.client.update_task(&todoist_id, &todoist_task).await?;

                    // Handle completion status changes
                    if task.completed {
                        let _ = self.client.close_task(&todoist_id).await;
                    } else {
                        let _ = self.client.reopen_task(&todoist_id).await;
                    }

                    let info = TaskSyncInfo {
                        todoist_id: todoist_id.clone(),
                        last_modified: Utc::now(),
                        last_sync_hash: self.compute_task_hash(&task),
                    };
                    self.metadata.update_mapping(yarmtl_id, info);
                }

                Ok(ActionType::UpdatedInTodoist)
            }
            SyncAction::UpdateYarmtl { todoist_id, task } => {
                let yarmtl_task = self.convert_todoist_to_yarmtl(&task);

                // Find and update the local task
                if let Some(local_task) = self.local_tasks.iter_mut().find(|t| t.id == yarmtl_task.id) {
                    *local_task = yarmtl_task.clone();
                    self.tasks_modified = true;
                }

                // Update metadata
                let info = TaskSyncInfo {
                    todoist_id,
                    last_modified: Utc::now(),
                    last_sync_hash: self.compute_task_hash(&yarmtl_task),
                };
                self.metadata.update_mapping(yarmtl_task.id, info);

                Ok(ActionType::UpdatedInYarmtl)
            }
            SyncAction::DeleteFromTodoist { todoist_id } => {
                self.client.delete_task(&todoist_id).await?;
                Ok(ActionType::DeletedFromTodoist)
            }
            SyncAction::DeleteFromYarmtl { yarmtl_id } => {
                // Remove from local tasks
                self.local_tasks.retain(|t| t.id != yarmtl_id);
                self.tasks_modified = true;

                self.metadata.remove_mapping(&yarmtl_id);
                Ok(ActionType::DeletedFromYarmtl)
            }
        }
    }

    async fn get_or_create_project(&mut self, project_name: &str) -> Option<String> {
        // Check if project already exists in cache
        if let Some(project_id) = self.projects.get(project_name) {
            return Some(project_id.clone());
        }

        // Create new project
        match self.client.create_project(project_name).await {
            Ok(project) => {
                self.projects.insert(project.name.clone(), project.id.clone());
                Some(project.id)
            }
            Err(e) => {
                eprintln!("⚠ Failed to create project '{}': {}", project_name, e);
                None
            }
        }
    }

    fn convert_yarmtl_to_todoist(&self, task: &Task) -> TodoistTask {
        let due = task.deadline.map(|d| TodoistDue {
            date: d.format("%Y-%m-%d").to_string(),
            datetime: None,
            timezone: None,
        });

        // First tag becomes project, rest become labels
        let (project_id, labels) = if task.tags.is_empty() {
            (None, None)
        } else {
            let project_name = &task.tags[0];
            let project_id = self.projects.get(project_name).cloned();

            // Rest of tags become labels (if any)
            let labels = if task.tags.len() > 1 {
                Some(task.tags[1..].to_vec())
            } else {
                None
            };

            (project_id, labels)
        };

        // Convert importance: yarmtl 1-5 (1=most) -> todoist 1-4 (4=most)
        let priority = task.importance.map(|i| match i {
            1 => 4,
            2 => 3,
            3 => 2,
            _ => 1,
        });

        let metadata = YarmtlMetadata {
            id: task.id.clone(),
            deadline: task.deadline.map(|d| d.format("%Y-%m-%d").to_string()),
            reminder: task.reminder.map(|r| r.format("%Y-%m-%d").to_string()),
            notes: task.notes.clone(),
            importance: task.importance,
        };

        let description = Some(metadata.encode());

        TodoistTask {
            id: None, // Will be set by Todoist
            content: task.text.clone(),
            description,
            due,
            labels,
            priority,
            is_completed: None, // Don't set here, use close_task/reopen_task instead
            project_id,
        }
    }

    fn convert_todoist_to_yarmtl(&self, todoist_task: &TodoistTask) -> Task {
        let metadata = todoist_task
            .description
            .as_ref()
            .and_then(|d| YarmtlMetadata::parse(d));

        let id = metadata
            .as_ref()
            .map(|m| m.id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string()[..8].to_string());

        // Prefer deadline from Todoist's due field, fall back to metadata
        let deadline = todoist_task
            .due
            .as_ref()
            .and_then(|d| NaiveDate::parse_from_str(&d.date, "%Y-%m-%d").ok())
            .or_else(|| {
                metadata
                    .as_ref()
                    .and_then(|m| m.deadline.as_ref())
                    .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
            });

        // Tags: project comes first, then labels
        let mut tags = Vec::new();

        // Add project name as first tag
        if let Some(project_id) = &todoist_task.project_id {
            // Find project name from project_id
            if let Some((name, _)) = self.projects.iter().find(|(_, id)| id == &project_id) {
                tags.push(name.clone());
            }
        }

        // Add labels as additional tags
        if let Some(labels) = &todoist_task.labels {
            tags.extend(labels.clone());
        }

        let reminder = metadata
            .as_ref()
            .and_then(|m| m.reminder.as_ref())
            .and_then(|r| NaiveDate::parse_from_str(r, "%Y-%m-%d").ok());

        let notes = metadata.as_ref().and_then(|m| m.notes.clone());

        let importance = metadata.as_ref().and_then(|m| m.importance);

        Task {
            id,
            text: todoist_task.content.clone(),
            deadline,
            tags,
            reminder,
            completed: todoist_task.is_completed.unwrap_or(false),
            notes,
            importance,
        }
    }

    fn extract_yarmtl_metadata(&self, todoist_task: &TodoistTask) -> Option<YarmtlMetadata> {
        todoist_task
            .description
            .as_ref()
            .and_then(|d| YarmtlMetadata::parse(d))
    }

    fn compute_task_hash(&self, task: &Task) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        task.text.hash(&mut hasher);
        task.deadline.hash(&mut hasher);
        task.tags.iter().for_each(|t| t.hash(&mut hasher));
        task.reminder.hash(&mut hasher);
        task.completed.hash(&mut hasher);
        if let Some(ref notes) = task.notes {
            notes.hash(&mut hasher);
        }
        task.importance.hash(&mut hasher);

        format!("{:x}", hasher.finish())
    }
}

enum ActionType {
    CreatedInTodoist,
    CreatedInYarmtl,
    UpdatedInTodoist,
    UpdatedInYarmtl,
    DeletedFromTodoist,
    DeletedFromYarmtl,
}
