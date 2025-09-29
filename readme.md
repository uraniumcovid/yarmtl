# YARMTL: Yet Another Rust Markdown Todo List

![logo](crab.png)

A powerful, markdown-based task manager with deadline tracking, email reminders, and smart natural language parsing. YARMTL stores your tasks in plain markdown files while providing a rich CLI interface with visual deadline indicators.

## ✨ Features

- **📝 Markdown-based storage** - Tasks stored in human-readable `tasks.md` files
- **📅 Smart deadline tracking** - Visual indicators for overdue, due today, and future deadlines
- **🔔 Flexible reminders** - Support for `@today`, `@tomorrow`, or specific dates
- **🏷️ Multiple tags** - Organize tasks with multiple hashtags (`#work #urgent #frontend`)
- **📧 Email notifications** - Automated daily reminders for due/overdue tasks
- **🤖 Daemon mode** - Run as background service for automatic notifications
- **🎨 Rich CLI interface** - Color-coded task display with emoji indicators
- **🔧 Automatic git versioning** - Every task change is automatically committed to git

## 🚀 Quick Start

### Installation

```bash
# Clone the repository
git clone <repository-url>
cd yarmtl

# Build the project
cargo build --release
```

### Basic Usage

```bash
# Add a simple task
./target/release/yarmtl "Buy groceries"

# Add a task with deadline and project
./target/release/yarmtl "Submit report !2025-10-01 #work"

# Add task with deadline and reminder
./target/release/yarmtl "Call dentist !2025-09-30 @tomorrow #personal"

# List all tasks
./target/release/yarmtl --list

# List including completed tasks
./target/release/yarmtl --list --done

# Use a custom directory for tasks (creates if doesn't exist)
./target/release/yarmtl --path ~/my-project-tasks "Project task #work"
./target/release/yarmtl --path ~/my-project-tasks --list
```

## 📋 Task Syntax

YARMTL uses an intuitive syntax for task metadata:

### Deadlines
```
!YYYY-MM-DD    # Deadline date
```

### Reminders
```
@YYYY-MM-DD    # Specific reminder date
@today         # Remind today
@tomorrow      # Remind tomorrow
```

### Tags
```
#tag-name      # Single tag
#work #urgent  # Multiple tags
```

### Complete Examples
```bash
# Basic task
"Finish presentation"

# Task with deadline
"Submit quarterly report !2025-10-15"

# Task with deadline and tags
"Review PR #142 !2025-09-30 #work #urgent"

# Task with everything
"Plan team meeting !2025-10-01 @today #work #management #weekly"
```

## 🎯 Visual Indicators

When listing tasks, YARMTL provides clear visual feedback:

- ⚠️ **Overdue** - Tasks past their deadline (red warning)
- 🔴 **Due Today** - Tasks due today (red dot)  
- 📅 **Future** - Upcoming deadlines (calendar icon)
- 🔔 **Reminders** - Tasks with reminder dates (bell icon)
- 🏷️ **Tags** - Multiple tags for organization (tag icon)
- ☐ **Incomplete** - Open checkbox for pending tasks
- ☑ **Complete** - Checked box for finished tasks

## 📧 Email Notifications

### Setup Email Configuration

```bash
# Generate email configuration template
./target/release/yarmtl --setup-email
```

Edit the generated `email_config.toml`:

```toml
smtp_server = "smtp.gmail.com"
smtp_port = 587
username = "your_email@gmail.com"
password = "your_app_password"  # Use app password for Gmail
from_email = "your_email@gmail.com"
to_email = "your_email@gmail.com"
```

### Send Manual Reminders

```bash
# Send email for all due/overdue tasks
./target/release/yarmtl --email
```

### Run as Daemon

```bash
# Start daemon - sends daily reminders at 5:00 AM
./target/release/yarmtl --daemon
```

The daemon will automatically send emails for:
- Tasks with deadlines today or overdue
- Tasks with reminder dates that have arrived

## 🔧 Git Integration

YARMTL automatically versions all your task changes with git:

- **Automatic initialization** - Creates a git repository on first use
- **Auto-commits** - Every task addition, completion, or deletion is committed
- **Clear commit messages** - Timestamped commits like "📝 Updated tasks - 2025-09-29 18:48:59"
- **Local versioning** - No remote repository required (but you can add one)

### Git History
```bash
# View your task change history
git log --oneline

# See what changed in a specific commit
git show <commit-hash>

# View task file changes over time
git log -p tasks.md
```

### Adding a Remote Repository
```bash
# Optional: Add your own remote repository
git remote add origin https://github.com/yourusername/my-tasks.git
git push -u origin main
```

## 📁 Custom Task Directories

YARMTL supports using custom directories for different projects or contexts:

### Project-specific Tasks
```bash
# Work tasks
yarmtl --path ~/tasks/work "Review PR #123 !tomorrow #work"
yarmtl --path ~/tasks/work --list

# Personal tasks  
yarmtl --path ~/tasks/personal "Buy groceries !today #personal"
yarmtl --path ~/tasks/personal --list

# Project-specific tasks
yarmtl --path ~/projects/website/tasks "Fix responsive design !2025-10-01 #frontend"
```

### Benefits of Custom Paths
- **Project isolation** - Keep different project tasks separate
- **Team collaboration** - Share project task directories via git
- **Organization** - Organize tasks by context (work/personal/project)
- **Git integration** - Each directory has its own git history
- **Email reminders** - Configure different email settings per directory

## 📁 File Structure

YARMTL creates and manages these files:

```
├── tasks.md              # Your task list (markdown format)
├── email_config.toml     # Email configuration (after setup)
└── .git/                 # Optional git repo for task history
```

### Example `tasks.md`:
```markdown
# tasks

- [ ] buy milk !2025-09-26 #groceries #shopping @today
- [ ] finish presentation !2025-09-27 #work #urgent
- [ ] call dentist #personal #health
- [x] submit report !2025-09-25 #work #quarterly
```

## 🔧 Command Line Options

```bash
USAGE:
    yarmtl [OPTIONS] [TASK]

ARGUMENTS:
    <TASK>    Task text to add (if empty, launch TUI)

OPTIONS:
    -l, --list           List all tasks
    -d, --done           Show completed tasks too
    -e, --email          Send email reminders for overdue/due tasks
        --setup-email    Setup email configuration
        --daemon         Run as daemon, sending emails at 5 AM daily
    -p, --path <DIR>     Path to directory containing tasks.md (creates if doesn't exist)
    -h, --help           Print help information
    -V, --version        Print version information
```

## 🎨 Example Workflow

```bash
# Start your day - check what's due
yarmtl --list

# Add some tasks
yarmtl "Review design docs !2025-10-01 @today #work"
yarmtl "Buy birthday gift !2025-10-05 #personal"
yarmtl "Team standup !2025-09-30 @tomorrow #work"

# Check updated task list
yarmtl --list

# Set up email notifications
yarmtl --setup-email
# Edit email_config.toml with your credentials

# Test email reminders
yarmtl --email

# Start daemon for automatic daily reminders
yarmtl --daemon
```

## 🛠 Development

### Dependencies

- `clap` - Command line argument parsing
- `chrono` - Date and time handling
- `regex` - Task parsing
- `lettre` - Email functionality
- `tokio` - Async runtime
- `tokio-cron-scheduler` - Daemon scheduling
- `serde` & `toml` - Configuration management

### Future Features

- [ ] TUI interface for interactive task management
- [ ] Task completion marking from CLI
- [ ] Git integration for task history
- [ ] Natural language date parsing
- [ ] Task priority levels
- [ ] Recurring task support
- [ ] Export to other formats

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

This project is open source. See LICENSE file for details.

---

*YARMTL - Because sometimes you need yet another way to manage your tasks! 🦀*
