# yarmtl - yet another rust markdown todo(ist) list
yarmtl is a todo app writting in rust, that stores your tasks in a simple markdown file.
the program supports the todoist api for 2-way sync, and git-based sync.

## installation

### using nix:

```bash
# run the installation script
./install.sh

# Or install manually
nix --extra-experimental-features nix-command --extra-experimental-features flakes profile install .
```

### from source

```bash
cargo build --release
cp target/release/yarmtl ~/.local/bin/
```

## basic commands

```bash
# open the tui:
yarmtl

# add task directly:
yarmtl "Fix the bug in module X #urgent !2024-12-20"

# print all tasks (excluding completed):
yarmtl --list

# print all tasks (including completed):
yarmtl --list --done

# give yarmtl your todoist api-key to use 2-way sync with todoist:
yarmtl --setup-todoist
```

## tui

### tui task management
- a/i: add new task ("add"/"insert", not ai bs)
- d/Del: delete selected task
- c: toggle show completed tasks
- r: reload tasks
- n: view task notes
- s: sync with todoist (requires api key from "yarmtl --setup-todoist" command above)
- t: toggle tags menu
- esc: clear tag filter

### tui navigation
- j/down: next task
- k/up: prev. task
- enter/space: toggle task completion

## task notation
```
yarmtl "task description !deadline #tag @reminder //notes $importance"
```

- `!2024-12-31` or `!tomorrow` - Set deadline
- `#work` - Add tags
- `@2024-12-25` or `@today` - Set reminder
- `//important notes` - Add notes
- `$5` - Set importance (1-5)

## github and todoist sync (warning!)
the system will attempt to store the todoist api key in the system key ring, but it will fallback to the local file share storage, if it can't acess the key ring.
do not git-version that backup api location, since you would risk exposing your api key to a public repo, if you for some reason used a public repo for storing the api key.
nb: this is not the same folder as the task storage itself. that can safely be git-versioned.

i am not responsible for any resulting problems from that or general usage of the software.

your tasks are automatically stored in `~/.local/share/yarmtl/yarmtl-tasks/tasks.md` with git versioning.

### todoist sync
to sync with todoist, you will need to use the "yarmtl --setup-todoist" command to supply an api key.
sync will be preformed by pressing "s" in the tui, as mentioned above.

### github sync
to sync with github:

```bash
cd ~/.local/share/yarmtl/yarmtl-tasks
git remote add origin https://github.com/yourusername/yarmtl-tasks.git
git push -u origin main
```
## development

```bash
# Enter development shell
nix develop

# Build
cargo build

# Run tests
cargo test
```
