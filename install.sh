#!/usr/bin/env bash

# YARMTL Installation Script
set -euo pipefail

echo "🦀 Installing YARMTL (Yet Another Rust Markdown Todo List)"

# Check if nix is available and flakes are enabled
if ! command -v nix &> /dev/null; then
    echo "❌ Nix is not installed. Please install Nix first:"
    echo "  curl -L https://nixos.org/nix/install | sh"
    exit 1
fi

# Check for flake support
if ! nix --extra-experimental-features nix-command --extra-experimental-features flakes --help &> /dev/null; then
    echo "❌ Nix flakes are not available. Please enable experimental features:"
    echo "  Add this to ~/.config/nix/nix.conf or /etc/nix/nix.conf:"
    echo "  experimental-features = nix-command flakes"
    exit 1
fi

# Create sync directory
SYNC_DIR="$HOME/.local/share/yarmtl/yarmtl-tasks"
echo "📁 Creating sync directory: $SYNC_DIR"
mkdir -p "$SYNC_DIR"

# Build and install
echo "🔨 Building YARMTL with Nix..."
nix --extra-experimental-features nix-command --extra-experimental-features flakes build

echo "📦 Removing any existing installations..."
nix --extra-experimental-features nix-command --extra-experimental-features flakes profile list | grep -q yarmtl && {
    nix --extra-experimental-features nix-command --extra-experimental-features flakes profile remove yarmtl || true
    nix --extra-experimental-features nix-command --extra-experimental-features flakes profile remove yarmtl-1 || true
}

echo "📦 Installing to user profile..."
nix --extra-experimental-features nix-command --extra-experimental-features flakes profile install .

# Initialize git repo in sync directory if needed
if [ ! -d "$SYNC_DIR/.git" ]; then
    echo "🔧 Initializing git repository in sync directory..."
    cd "$SYNC_DIR"
    git init
    git config user.email "yarmtl@local"
    git config user.name "YARMTL"
    echo "📝 Git repository initialized in $SYNC_DIR"
    echo "💡 To enable auto-push to GitHub, set up a remote:"
    echo "   cd $SYNC_DIR"
    echo "   git remote add origin https://github.com/yourusername/yarmtl-tasks.git"
    echo "   git branch -M main"
    echo "   git push -u origin main"
    echo "🔄 After setup, tasks will auto-commit AND auto-push on every change!"
fi

echo "✅ Installation complete!"
echo ""
echo "📋 Usage:"
echo "  yarmtl                    # Launch TUI"
echo "  yarmtl 'task description' # Add a task"
echo "  yarmtl --list             # List tasks"
echo "  yarmtl --help             # Show help"
echo ""
echo "📂 Your tasks are stored in: $SYNC_DIR/tasks.md"
echo "🔄 To sync with GitHub, set up a remote in: $SYNC_DIR"