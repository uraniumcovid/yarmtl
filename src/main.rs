// first add to Cargo.toml:
// clap = { version = "4.0", features = ["derive"] }

mod tui;

use clap::Parser;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::env;
use chrono::{Local, NaiveDate};
use regex::Regex;
use chrono_english::{parse_date_string, Dialect};
use serde::{Deserialize, Serialize};
use lettre::{Message, SmtpTransport, Transport};
use lettre::transport::smtp::authentication::Credentials;
use tokio_cron_scheduler::{JobScheduler, Job};
use std::sync::OnceLock;
use uuid::Uuid;

// Global state for working directory
static WORKING_DIR: OnceLock<PathBuf> = OnceLock::new();

fn set_working_dir(path: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let work_dir = if let Some(p) = path {
        let pb = PathBuf::from(p);
        if !pb.exists() {
            fs::create_dir_all(&pb)?;
            println!("📁 Created directory: {}", pb.display());
        }
        if !pb.is_dir() {
            return Err(format!("Path {} is not a directory", pb.display()).into());
        }
        pb.canonicalize()?
    } else {
        env::current_dir()?
    };
    
    let _ = WORKING_DIR.set(work_dir);
    Ok(())
}

fn get_working_dir() -> PathBuf {
    WORKING_DIR.get().cloned().unwrap_or_else(|| env::current_dir().unwrap())
}

fn get_tasks_file_path() -> PathBuf {
    get_working_dir().join("tasks.md")
}

fn get_email_config_path() -> PathBuf {
    get_working_dir().join("email_config.toml")
}


#[derive(Deserialize, Serialize)]
struct EmailConfig {
    smtp_server: String,
    smtp_port: u16,
    username: String,
    password: String,
    from_email: String,
    to_email: String,
}

impl Default for EmailConfig {
    fn default() -> Self {
        EmailConfig {
            smtp_server: "smtp.gmail.com".to_string(),
            smtp_port: 587,
            username: "your_email@gmail.com".to_string(),
            password: "your_app_password".to_string(),
            from_email: "your_email@gmail.com".to_string(),
            to_email: "your_email@gmail.com".to_string(),
        }
    }
}

#[derive(Parser)]
#[command(name = "yarmtl")]
#[command(author, version, about = "yet another rust markdown todo list", long_about = None)]
struct Cli {
    /// task text to add (if empty, launch tui)
    task: Option<String>,
    
    /// list all tasks
    #[arg(short, long)]
    list: bool,
    
    /// show completed tasks too
    #[arg(short, long)]
    done: bool,
    
    /// send email reminders for overdue/due tasks
    #[arg(short, long)]
    email: bool,
    
    /// setup email configuration
    #[arg(long)]
    setup_email: bool,
    
    /// run as daemon, sending emails at 5 AM daily
    #[arg(long)]
    daemon: bool,
    
    /// path to directory containing tasks.md (creates if doesn't exist)
    #[arg(short, long, value_name = "DIR")]
    path: Option<String>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    
    // Set up working directory first
    if let Err(e) = set_working_dir(cli.path.as_deref()) {
        eprintln!("Error setting up working directory: {}", e);
        return;
    }
    
    if cli.path.is_some() {
        println!("📂 Working directory: {}", get_working_dir().display());
    }
    
    if cli.setup_email {
        setup_email_config();
        return;
    }
    
    if cli.daemon {
        if let Err(e) = run_daemon().await {
            eprintln!("Daemon failed: {}", e);
        }
        return;
    }
    
    if cli.email {
        if let Err(e) = send_email_reminders().await {
            eprintln!("Failed to send email reminders: {}", e);
        }
        return;
    }
    
    match cli.task {
        Some(text) => {
            println!("adding task: {}", text);
            add_task(&text);
        }
        None => {
            if cli.list {
                list_tasks(cli.done);
            } else {
                println!("🚀 Launching YARMTL TUI...");
                if let Err(e) = tui::run_tui(&get_working_dir()) {
                    eprintln!("TUI failed: {}", e);
                }
            }
        }
    }
}

pub fn add_task(text: &str) {
    let task_file = get_tasks_file_path();
    
    if !task_file.exists() {
        fs::write(&task_file, "# tasks\n\n").expect("couldn't create tasks file");
    }
    
    let mut content = fs::read_to_string(&task_file)
        .expect("couldn't read tasks file");
    
    // Check if this is a subtask using the "<-" syntax
    if text.trim_start().starts_with("<-") {
        let subtask_text = text.trim_start().strip_prefix("<-").unwrap().trim();
        add_subtask_to_last_task(&mut content, subtask_text);
    } else {
        let task = Task::parse(text);
        let new_task = format!("{}\n", task.to_markdown());
        content.push_str(&new_task);
    }
    
    fs::write(&task_file, content)
        .expect("couldn't write tasks file");
    
    // Auto-commit the task addition with descriptive message
    let commit_message = if text.trim_start().starts_with("<-") {
        format!("➕ Added subtask: \"{}\"", text.trim_start().strip_prefix("<-").unwrap().trim())
    } else {
        let task = Task::parse(text);
        format!("➕ Added task: \"{}\"", task.text)
    };
    
    if let Err(e) = git_commit_tasks_with_message(Some(&commit_message)) {
        eprintln!("Warning: Failed to commit task to git: {}", e);
    }
    
    if text.trim_start().starts_with("<-") {
        let subtask_text = text.trim_start().strip_prefix("<-").unwrap().trim();
        println!("✓ added subtask: \"{}\"", subtask_text);
    } else {
        let task = Task::parse(text);
        println!("✓ added task: \"{}\"", task.text);
        if let Some(deadline) = task.deadline {
            println!("  📅 deadline: {}", deadline.format("%Y-%m-%d"));
        }
        if !task.tags.is_empty() {
            println!("  🏷️  tags: {}", task.tags.iter().map(|t| format!("#{}", t)).collect::<Vec<_>>().join(" "));
        }
        if let Some(reminder) = task.reminder {
            println!("  🔔 reminder: {}", reminder.format("%Y-%m-%d"));
        }
    }
}

fn add_subtask_to_last_task(content: &mut String, subtask_text: &str) {
    let lines: Vec<&str> = content.lines().collect();
    let mut last_task_line_index = None;
    let mut last_task_id = None;
    
    // Find the last task line and extract its ID
    for (i, line) in lines.iter().enumerate().rev() {
        if line.trim().starts_with("- [ ]") || line.trim().starts_with("- [x]") {
            last_task_line_index = Some(i);
            // Extract ID from the last task
            let id_re = Regex::new(r"\[id:([a-f0-9-]+)\]").unwrap();
            if let Some(captures) = id_re.captures(line) {
                last_task_id = Some(captures[1].to_string());
            }
            break;
        }
    }
    
    if let (Some(line_index), Some(parent_id)) = (last_task_line_index, last_task_id) {
        // Create the subtask with parent ID and indent
        let subtask = Task::parse_with_parent(subtask_text, Some(parent_id));
        let subtask_markdown = format!("  {}", subtask.to_markdown());
        
        // Insert the subtask after the parent task
        let mut new_lines = lines[..=line_index].to_vec();
        new_lines.push(&subtask_markdown);
        new_lines.extend_from_slice(&lines[line_index + 1..]);
        
        *content = new_lines.join("\n");
        if !content.ends_with('\n') {
            content.push('\n');
        }
    }
}

pub fn list_tasks(show_completed: bool) {
    let task_file = get_tasks_file_path();
    
    if !task_file.exists() {
        println!("no tasks file found. add a task first!");
        return;
    }
    
    let content = fs::read_to_string(&task_file)
        .expect("couldn't read tasks file");
    
    println!("tasks:");
    
    for line in content.lines() {
        let trimmed_line = line.trim_start();
        if trimmed_line.starts_with("- [ ]") {
            let indent_level = (line.len() - line.trim_start().len()) / 2;
            let task_text = trimmed_line.strip_prefix("- [ ] ").unwrap_or(trimmed_line);
            let task = Task::parse(task_text);
            
            let indent_prefix = "  ".repeat(indent_level + 1); // +1 for base indentation
            print!("{}☐ {}", indent_prefix, task.text);
            if let Some(deadline) = task.deadline {
                let today = chrono::Local::now().date_naive();
                if deadline < today {
                    print!(" ⚠️ !{} (overdue)", deadline.format("%Y-%m-%d"));
                } else if deadline == today {
                    print!(" 🔴 !{} (due today)", deadline.format("%Y-%m-%d"));
                } else {
                    print!(" 📅 !{}", deadline.format("%Y-%m-%d"));
                }
            }
            if !task.tags.is_empty() {
                for tag in &task.tags {
                    print!(" 🏷️ #{}", tag);
                }
            }
            if let Some(reminder) = task.reminder {
                print!(" 🔔 @{}", reminder.format("%Y-%m-%d"));
            }
            // Notes displayed last like a comment
            if let Some(ref notes) = task.notes {
                print!(" //{}", notes);
            }
            println!();
        } else if trimmed_line.starts_with("- [x]") && show_completed {
            let indent_level = (line.len() - line.trim_start().len()) / 2;
            let task_text = trimmed_line.strip_prefix("- [x] ").unwrap_or(trimmed_line);
            let task = Task::parse(task_text);
            
            let indent_prefix = "  ".repeat(indent_level + 1); // +1 for base indentation
            print!("{}☑ {}", indent_prefix, task.text);
            if let Some(deadline) = task.deadline {
                print!(" 📅 !{}", deadline.format("%Y-%m-%d"));
            }
            if !task.tags.is_empty() {
                for tag in &task.tags {
                    print!(" 🏷️ #{}", tag);
                }
            }
            if let Some(reminder) = task.reminder {
                print!(" 🔔 @{}", reminder.format("%Y-%m-%d"));
            }
            // Notes displayed last like a comment
            if let Some(ref notes) = task.notes {
                print!(" //{}", notes);
            }
            println!();
        }
    }
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub text: String,
    pub deadline: Option<NaiveDate>,
    pub tags: Vec<String>,
    pub reminder: Option<NaiveDate>,
    pub completed: bool,
    pub notes: Option<String>,
    pub subtasks: Vec<Task>,
    pub indent_level: usize,
    pub parent_id: Option<String>,
}

impl Task {
    pub fn parse(input: &str) -> Self {
        Self::parse_with_indent(input, 0)
    }

    pub fn parse_with_parent(input: &str, parent_id: Option<String>) -> Self {
        Self::parse_with_parent_and_indent(input, parent_id, 0)
    }

    pub fn parse_with_indent(input: &str, indent_level: usize) -> Self {
        Self::parse_with_parent_and_indent(input, None, indent_level)
    }

    pub fn parse_with_parent_and_indent(input: &str, parent_id: Option<String>, indent_level: usize) -> Self {
        let deadline_re = Regex::new(r"!(\d{4}-\d{2}-\d{2})").unwrap();
        let tags_re = Regex::new(r"#(\w+)").unwrap();
        let reminder_date_re = Regex::new(r"@(\d{4}-\d{2}-\d{2})").unwrap();
        let id_re = Regex::new(r"\[id:([a-f0-9-]+)\]").unwrap();
        
        // Use a simpler approach: parse notes with regex that stops at metadata
        let notes_re = Regex::new(r"//([^!@#]+)").unwrap();
        let notes = notes_re.find(input)
            .map(|m| m.as_str().trim_start_matches("//").trim().to_string())
            .filter(|s| !s.is_empty());
        
        // Extract existing ID or generate new one
        let task_id = id_re.find(input)
            .map(|m| m.as_str().trim_start_matches("[id:").trim_end_matches("]").to_string())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        
        let deadline = deadline_re.find(input)
            .and_then(|m| NaiveDate::parse_from_str(m.as_str().trim_start_matches('!'), "%Y-%m-%d").ok())
            .or_else(|| {
                // Try natural language parsing for deadlines
                Self::extract_natural_deadline(input)
            });
        
        // Extract all tags (multiple #tags)
        let tags: Vec<String> = tags_re.find_iter(input)
            .map(|m| m.as_str().trim_start_matches('#').to_string())
            .collect();
        
        let reminder = reminder_date_re.find(input)
            .and_then(|m| NaiveDate::parse_from_str(m.as_str().trim_start_matches('@'), "%Y-%m-%d").ok())
            .or_else(|| {
                // Try natural language parsing for reminders
                Self::extract_natural_reminder(input)
            });
        
        let mut clean_text = input.to_string();
        clean_text = deadline_re.replace_all(&clean_text, "").to_string();
        clean_text = Self::remove_natural_deadline(&clean_text);
        clean_text = tags_re.replace_all(&clean_text, "").to_string();
        clean_text = reminder_date_re.replace_all(&clean_text, "").to_string();
        clean_text = Self::remove_natural_reminder(&clean_text);
        clean_text = notes_re.replace_all(&clean_text, "").to_string();
        clean_text = id_re.replace_all(&clean_text, "").to_string();
        clean_text = clean_text.trim().to_string();
        
        Task {
            id: task_id,
            text: clean_text,
            deadline,
            tags,
            reminder,
            completed: false,
            notes,
            subtasks: Vec::new(),
            indent_level,
            parent_id,
        }
    }
    
    pub fn to_markdown(&self) -> String {
        let checkbox = if self.completed { "[x]" } else { "[ ]" };
        let indent = "  ".repeat(self.indent_level);
        let mut result = format!("{}- {} {} [id:{}]", indent, checkbox, self.text, &self.id[..8]);
        
        if let Some(ref deadline) = self.deadline {
            result.push_str(&format!(" !{}", deadline.format("%Y-%m-%d")));
        }
        
        for tag in &self.tags {
            result.push_str(&format!(" #{}", tag));
        }
        
        if let Some(ref reminder) = self.reminder {
            result.push_str(&format!(" @{}", reminder.format("%Y-%m-%d")));
        }

        if let Some(ref notes) = self.notes {
            result.push_str(&format!(" //{}", notes));
        }
        
        result
    }

    fn extract_natural_deadline(input: &str) -> Option<NaiveDate> {
        // Find text after ! that isn't a date format
        if let Some(start) = input.find('!') {
            let after_exclaim = &input[start + 1..];
            
            // Find the end of the deadline phrase (before #, @, //, or end of string)
            let end_pos = after_exclaim
                .find("//")
                .or_else(|| after_exclaim.find(|c| c == '#' || c == '@'))
                .unwrap_or(after_exclaim.len());
            
            let deadline_text = after_exclaim[..end_pos].trim();
            
            if !deadline_text.is_empty() && !deadline_text.chars().all(|c| c.is_digit(10) || c == '-') {
                match deadline_text {
                    "today" => return Some(chrono::Local::now().date_naive()),
                    "tomorrow" => return Some(chrono::Local::now().date_naive() + chrono::Duration::days(1)),
                    "yesterday" => return Some(chrono::Local::now().date_naive() - chrono::Duration::days(1)),
                    _ => {
                        // Try parsing with chrono-english
                        if let Ok(parsed_date) = parse_date_string(deadline_text, Local::now(), Dialect::Us) {
                            return Some(parsed_date.date_naive());
                        }
                    }
                }
            }
        }
        None
    }

    fn extract_natural_reminder(input: &str) -> Option<NaiveDate> {
        // Find text after @ that isn't a date format
        if let Some(start) = input.find('@') {
            let after_at = &input[start + 1..];
            
            // Find the end of the reminder phrase (before #, !, //, or end of string)
            let end_pos = after_at
                .find("//")
                .or_else(|| after_at.find(|c| c == '#' || c == '!'))
                .unwrap_or(after_at.len());
            
            let reminder_text = after_at[..end_pos].trim();
            
            if !reminder_text.is_empty() && !reminder_text.chars().all(|c| c.is_digit(10) || c == '-') {
                match reminder_text {
                    "today" => return Some(chrono::Local::now().date_naive()),
                    "tomorrow" => return Some(chrono::Local::now().date_naive() + chrono::Duration::days(1)),
                    "yesterday" => return Some(chrono::Local::now().date_naive() - chrono::Duration::days(1)),
                    _ => {
                        // Try parsing with chrono-english
                        if let Ok(parsed_date) = parse_date_string(reminder_text, Local::now(), Dialect::Us) {
                            return Some(parsed_date.date_naive());
                        }
                    }
                }
            }
        }
        None
    }

    fn remove_natural_deadline(input: &str) -> String {
        if let Some(start) = input.find('!') {
            let before = &input[..start];
            let after_exclaim = &input[start + 1..];
            
            let end_pos = after_exclaim
                .find("//")
                .or_else(|| after_exclaim.find(|c| c == '#' || c == '@'))
                .unwrap_or(after_exclaim.len());
            
            let deadline_text = after_exclaim[..end_pos].trim();
            
            if !deadline_text.is_empty() && !deadline_text.chars().all(|c| c.is_digit(10) || c == '-') {
                // Remove the natural language deadline
                let after = &after_exclaim[end_pos..];
                return format!("{}{}", before, after);
            }
        }
        input.to_string()
    }

    fn remove_natural_reminder(input: &str) -> String {
        if let Some(start) = input.find('@') {
            let before = &input[..start];
            let after_at = &input[start + 1..];
            
            let end_pos = after_at
                .find("//")
                .or_else(|| after_at.find(|c| c == '#' || c == '!'))
                .unwrap_or(after_at.len());
            
            let reminder_text = after_at[..end_pos].trim();
            
            if !reminder_text.is_empty() && !reminder_text.chars().all(|c| c.is_digit(10) || c == '-') {
                // Remove the natural language reminder
                let after = &after_at[end_pos..];
                return format!("{}{}", before, after);
            }
        }
        input.to_string()
    }

}

pub fn git_repo_check() -> Result<(), String> {
    let git_dir = get_working_dir().join(".git");
    if !git_dir.exists() {
        Command::new("git")
            .args(["init"])
            .current_dir(get_working_dir())
            .output()
            .map_err(|e| format!("failed to initialize git: {}", e))?;

        println!("🔧 Initialized git repository for task versioning");
        
        // Set git user if not configured
        let _ = Command::new("git")
            .args(["config", "user.email", "yarmtl@local"])
            .current_dir(get_working_dir())
            .output();
        
        let _ = Command::new("git")
            .args(["config", "user.name", "YARMTL"])
            .current_dir(get_working_dir())
            .output();
        
        // Create initial commit if tasks.md exists
        let tasks_file = get_tasks_file_path();
        if tasks_file.exists() {
            let add_result = Command::new("git")
                .args(["add", "tasks.md"])
                .current_dir(get_working_dir())
                .output()
                .map_err(|e| format!("git add failed: {}", e))?;

            if !add_result.status.success() {
                let error = String::from_utf8_lossy(&add_result.stderr);
                eprintln!("Warning: git add failed: {}", error);
                return Ok(()); // Don't fail, just warn
            }

            let commit_result = Command::new("git")
                .args(["commit", "-m", "🎉 Initial YARMTL tasks commit"])
                .current_dir(get_working_dir())
                .output()
                .map_err(|e| format!("git initial commit failed: {}", e))?;
            
            if !commit_result.status.success() {
                let error = String::from_utf8_lossy(&commit_result.stderr);
                eprintln!("Warning: git initial commit failed: {}", error);
                return Ok(()); // Don't fail, just warn
            }
            
            println!("📝 Created initial tasks commit");
        }
    }
    Ok(())
}

pub fn git_commit_tasks() -> Result<(), String> {
    git_commit_tasks_with_message(None)
}

pub fn git_commit_tasks_with_message(custom_message: Option<&str>) -> Result<(), String> {
    git_repo_check()?;

    let add_result = Command::new("git")
        .args(["add", "tasks.md"])
        .current_dir(get_working_dir())
        .output()
        .map_err(|e| format!("git add failed: {}", e))?;

    if !add_result.status.success() {
        let error = String::from_utf8_lossy(&add_result.stderr);
        return Err(format!("git add failed: {}", error));
    }

    // Check if there are changes to commit
    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(get_working_dir())
        .output()
        .map_err(|e| format!("git status failed: {}", e))?;

    if status_output.stdout.is_empty() {
        // No changes to commit
        return Ok(());
    }

    let message = if let Some(custom_msg) = custom_message {
        custom_msg.to_string()
    } else {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        format!("📝 Updated tasks - {}", timestamp)
    };

    let commit_result = Command::new("git")
        .args(["commit", "-m", &message])
        .current_dir(get_working_dir())
        .output()
        .map_err(|e| format!("git commit failed: {}", e))?;

    if !commit_result.status.success() {
        let error = String::from_utf8_lossy(&commit_result.stderr);
        return Err(format!("git commit failed: {}", error));
    }

    Ok(())
}

fn load_email_config() -> Result<EmailConfig, Box<dyn std::error::Error>> {
    let config_file = get_email_config_path();
    if !config_file.exists() {
        return Err("Email config file not found. Run with --setup-email first.".into());
    }
    
    let content = fs::read_to_string(config_file)?;
    let config: EmailConfig = toml::from_str(&content)?;
    Ok(config)
}

fn setup_email_config() {
    println!("Setting up email configuration...");
    
    let config = EmailConfig::default();
    let toml_content = toml::to_string_pretty(&config).unwrap();
    let config_file = get_email_config_path();
    
    fs::write(config_file, toml_content)
        .expect("couldn't write email config file");
    
    println!("✓ Created email_config.toml in {}", get_working_dir().display());
    println!("Please edit email_config.toml with your email settings:");
    println!("  - For Gmail: Use app password, not regular password");
    println!("  - smtp_server: Your SMTP server (e.g., smtp.gmail.com)");
    println!("  - smtp_port: Usually 587 for TLS");
    println!("  - username/password: Your email credentials");
    println!("  - from_email/to_email: Sender and recipient emails");
}

async fn run_daemon() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔄 Starting YARMTL daemon...");
    println!("📧 Email reminders will be sent at 5:00 AM daily");
    println!("📝 Checking for tasks with deadlines and reminder dates");
    println!("💡 Press Ctrl+C to stop");
    
    let sched = JobScheduler::new().await?;
    
    let job = Job::new_async("0 5 * * *", |_uuid, _l| {
        Box::pin(async {
            println!("[{}] Running daily email check...", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
            if let Err(e) = send_email_reminders().await {
                eprintln!("Failed to send email reminders: {}", e);
            }
        })
    })?;
    
    sched.add(job).await?;
    sched.start().await?;
    
    // Keep the daemon running
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}

async fn send_email_reminders() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_email_config()?;
    let task_file = get_tasks_file_path();
    
    if !task_file.exists() {
        println!("No tasks file found.");
        return Ok(());
    }
    
    let content = fs::read_to_string(task_file)?;
    let today = chrono::Local::now().date_naive();
    let mut reminder_tasks = Vec::new();
    
    for line in content.lines() {
        if line.starts_with("- [ ]") {
            let task_text = line.strip_prefix("- [ ] ").unwrap_or(line);
            let task = Task::parse(task_text);
            
            let mut should_remind = false;
            let mut reminder_reason = String::new();
            
            // Check deadline
            if let Some(deadline) = task.deadline {
                if deadline <= today {
                    should_remind = true;
                    if deadline < today {
                        reminder_reason = "deadline overdue".to_string();
                    } else {
                        reminder_reason = "deadline due today".to_string();
                    }
                }
            }
            
            // Check reminder date
            if let Some(reminder_date) = task.reminder {
                if reminder_date <= today && !should_remind {
                    should_remind = true;
                    reminder_reason = "reminder date reached".to_string();
                }
            }
            
            if should_remind {
                reminder_tasks.push((task, reminder_reason));
            }
        }
    }
    
    if reminder_tasks.is_empty() {
        println!("No tasks requiring reminders found.");
        return Ok(());
    }
    
    let mut email_body = String::from("Task Reminders\n\n");
    
    for (task, reason) in &reminder_tasks {
        email_body.push_str(&format!("📌 {}: {}\n", reason.to_uppercase(), task.text));
        if let Some(ref deadline) = task.deadline {
            email_body.push_str(&format!("  📅 Deadline: {}\n", deadline.format("%Y-%m-%d")));
        }
        if let Some(ref reminder) = task.reminder {
            email_body.push_str(&format!("  🔔 Reminder: {}\n", reminder.format("%Y-%m-%d")));
        }
        if !task.tags.is_empty() {
            email_body.push_str(&format!("  🏷️  Tags: {}\n", 
                task.tags.iter().map(|t| format!("#{}", t)).collect::<Vec<_>>().join(" ")));
        }
        email_body.push('\n');
    }
    
    let email = Message::builder()
        .from(config.from_email.parse()?)
        .to(config.to_email.parse()?)
        .subject("Task Reminders - YARMTL")
        .body(email_body)?;
    
    let creds = Credentials::new(config.username, config.password);
    let mailer = SmtpTransport::relay(&config.smtp_server)?
        .credentials(creds)
        .build();
    
    match mailer.send(&email) {
        Ok(_) => {
            println!("✓ Email reminders sent successfully!");
            println!("Sent {} reminder(s)", reminder_tasks.len());
        }
        Err(e) => {
            return Err(format!("Failed to send email: {}", e).into());
        }
    }
    
    Ok(())
}
