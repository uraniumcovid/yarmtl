# YARMTL - Yet Another Rust Markdown Todo List

A terminal-based todo list manager with git synchronization and GitHub sync support.

## Features

- 📝 Markdown-based task storage
- 🔄 Automatic git versioning
- 📅 Deadline and reminder support
- 🏷️ Tag-based organization
- 📧 Email reminders
- 🖥️ Terminal UI (TUI)
- ☁️ GitHub sync support

## Installation

### Using Nix (Recommended)

```bash
# Run the installation script
./install.sh

# Or install manually
nix --extra-experimental-features nix-command --extra-experimental-features flakes profile install .
```

### From Source

```bash
cargo build --release
cp target/release/yarmtl ~/.local/bin/
```

## Usage

### Basic Commands

```bash
# Launch interactive TUI
yarmtl

# Add a task
yarmtl "Fix the bug in module X #urgent !2024-12-20"

# List all tasks
yarmtl --list

# List including completed tasks
yarmtl --list --done
```

### Task Syntax

```
yarmtl "Task description !deadline #tag @reminder //notes $importance"
```

- `!2024-12-31` or `!tomorrow` - Set deadline
- `#work` - Add tags
- `@2024-12-25` or `@today` - Set reminder
- `//important notes` - Add notes
- `$5` - Set importance (1-5)

### GitHub Sync Setup

Your tasks are automatically stored in `~/.local/share/yarmtl/yarmtl-tasks/tasks.md` with git versioning.

To sync with GitHub:

```bash
cd ~/.local/share/yarmtl/yarmtl-tasks
git remote add origin https://github.com/yourusername/yarmtl-tasks.git
git push -u origin main
```

### Email Reminders

```bash
# Setup email configuration
yarmtl --setup-email

# Send reminders manually
yarmtl --email

# Run as daemon (sends at 5 AM daily)
yarmtl --daemon
```

## Development

```bash
# Enter development shell
nix develop

# Build
cargo build

# Run tests
cargo test
```