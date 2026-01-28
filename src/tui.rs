use crate::{Task, git_commit_tasks_with_message, is_todoist_sync_enabled, trigger_todoist_sync};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
    },
    Frame, Terminal,
};
use std::{
    fs,
    io,
    path::PathBuf,
};

pub struct App {
    pub tasks: Vec<Task>,
    pub list_state: ListState,
    pub input_mode: InputMode,
    pub input: String,
    pub show_completed: bool,
    pub show_help: bool,
    pub show_splash: bool,
    pub splash_timer: std::time::Instant,
    pub show_notes: bool,
    pub selected_task_for_notes: Option<usize>,
    pub working_dir: PathBuf,
    pub view_mode: ViewMode,
    pub tags_list_state: ListState,
    pub selected_tag: Option<String>,
    pub sync_status: Option<String>,
}

#[derive(Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Editing,
}

#[derive(Clone, PartialEq)]
pub enum ViewMode {
    Tasks,
    TagsMenu,
}

impl Default for App {
    fn default() -> App {
        App {
            tasks: Vec::new(),
            list_state: ListState::default(),
            input_mode: InputMode::Normal,
            input: String::new(),
            show_completed: false,
            show_help: false,
            show_splash: true,
            splash_timer: std::time::Instant::now(),
            show_notes: false,
            selected_task_for_notes: None,
            working_dir: std::env::current_dir().unwrap(),
            view_mode: ViewMode::Tasks,
            tags_list_state: ListState::default(),
            selected_tag: None,
            sync_status: None,
        }
    }
}

impl App {
    pub fn new(working_dir: &PathBuf) -> App {
        let mut app = App::default();
        app.working_dir = working_dir.clone();
        
        // Check if this is the first run
        let settings_file = working_dir.join(".yarmtl_settings");
        let is_first_run = !settings_file.exists();
        
        if is_first_run {
            // Create settings file to track that app has been run
            let _ = std::fs::write(&settings_file, "first_run_completed=true\n");
            app.show_splash = true;
        } else {
            app.show_splash = false;
        }
        
        app.load_tasks();
        if !app.tasks.is_empty() {
            app.list_state.select(Some(0));
        }
        app
    }

    pub fn load_tasks(&mut self) {
        let task_file = self.working_dir.join("tasks.md");
        
        if !task_file.exists() {
            return;
        }
        
        let content = match fs::read_to_string(&task_file) {
            Ok(content) => content,
            Err(_) => return,
        };
        
        self.tasks.clear();
        for line in content.lines() {
            // Count leading spaces to determine indentation level
            let _indent_level = line.chars().take_while(|&c| c == ' ').count() / 2;
            let trimmed_line = line.trim_start();
            
            if trimmed_line.starts_with("- [ ]") || trimmed_line.starts_with("- [x]") {
                let completed = trimmed_line.starts_with("- [x]");
                let task_text = if completed {
                    trimmed_line.strip_prefix("- [x] ").unwrap_or(trimmed_line)
                } else {
                    trimmed_line.strip_prefix("- [ ] ").unwrap_or(trimmed_line)
                };
                
                let mut task = Task::parse(task_text);
                task.completed = completed;
                self.tasks.push(task);
            }
        }
    }


    pub fn save_tasks_with_message(&self, commit_message: Option<&str>) {
        let task_file = self.working_dir.join("tasks.md");
        let mut content = String::from("# tasks\n\n");

        for task in &self.tasks {
            content.push_str(&format!("{}\n", task.to_markdown()));
        }

        let _ = fs::write(&task_file, content);

        // Auto-commit the task changes with custom message
        if let Err(e) = git_commit_tasks_with_message(commit_message) {
            eprintln!("Warning: Failed to commit tasks to git: {}", e);
        }

        // Trigger Todoist sync in background
        if is_todoist_sync_enabled() {
            tokio::spawn(async move {
                if let Err(e) = trigger_todoist_sync().await {
                    eprintln!("⚠ Todoist sync failed: {}", e);
                }
            });
        }
    }

    pub fn next_task(&mut self) {
        let total_items = self.get_total_display_items();
        if total_items == 0 {
            return;
        }
        
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= total_items - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn previous_task(&mut self) {
        let total_items = self.get_total_display_items();
        if total_items == 0 {
            return;
        }
        
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    total_items - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn toggle_completed(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if let Some(task_index) = self.get_task_index_from_display_position(selected) {
                let task = &mut self.tasks[task_index];
                task.completed = !task.completed;
                
                let action = if task.completed { "✅ Marked task complete" } else { "⏳ Marked task incomplete" };
                let commit_message = format!("{}: \"{}\"", action, task.text);
                self.save_tasks_with_message(Some(&commit_message));
            }
        }
    }

    pub fn add_new_task(&mut self) {
        if !self.input.trim().is_empty() {
            let new_task = Task::parse(&self.input);
            let task_text = new_task.text.clone();
            self.tasks.push(new_task);
            
            let commit_message = format!("➕ Added task: \"{}\"", task_text);
            self.save_tasks_with_message(Some(&commit_message));
            
            self.input.clear();
            self.input_mode = InputMode::Normal;
            
            // Select the new task
            let visible_tasks = self.get_visible_tasks();
            if !visible_tasks.is_empty() {
                self.list_state.select(Some(visible_tasks.len() - 1));
            }
        }
    }

    pub fn get_visible_tasks(&self) -> Vec<usize> {
        self.tasks
            .iter()
            .enumerate()
            .filter(|(_, task)| self.show_completed || !task.completed)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn get_grouped_tasks(&self) -> Vec<(String, Vec<usize>)> {
        let today = chrono::Local::now().date_naive();
        let mut overdue_today = Vec::new();
        let mut upcoming = Vec::new();
        let mut no_deadline = Vec::new();

        for (i, task) in self.tasks.iter().enumerate() {
            if !self.show_completed && task.completed {
                continue;
            }

            match task.deadline {
                Some(deadline) if deadline <= today => overdue_today.push(i),
                Some(_) => upcoming.push(i),
                None => no_deadline.push(i),
            }
        }

        // Sort upcoming tasks by deadline
        upcoming.sort_by(|&a, &b| {
            self.tasks[a].deadline.cmp(&self.tasks[b].deadline)
        });

        let mut result = Vec::new();
        
        if !overdue_today.is_empty() {
            result.push(("OVERDUE & TODAY".to_string(), overdue_today));
        }
        
        if !upcoming.is_empty() {
            result.push(("UPCOMING".to_string(), upcoming));
        }
        
        if !no_deadline.is_empty() {
            result.push(("NO DEADLINE".to_string(), no_deadline));
        }

        result
    }

    pub fn get_total_display_items(&self) -> usize {
        let grouped_tasks = self.get_grouped_tasks();
        let mut count = 0;
        
        for (_, task_indices) in grouped_tasks {
            if !task_indices.is_empty() {
                count += 1; // Section header
                count += task_indices.len(); // Tasks
                count += 1; // Spacing after section
            }
        }
        
        count
    }

    pub fn get_task_index_from_display_position(&self, display_pos: usize) -> Option<usize> {
        let grouped_tasks = self.get_grouped_tasks();
        let mut current_pos = 0;
        
        for (_, task_indices) in grouped_tasks {
            if !task_indices.is_empty() {
                // Skip section header
                current_pos += 1;
                
                // Check if we're in the task range for this section
                for &task_idx in &task_indices {
                    if current_pos == display_pos {
                        return Some(task_idx);
                    }
                    current_pos += 1;
                }
                
                // Skip spacing after section
                current_pos += 1;
            }
        }
        
        None
    }

    pub fn delete_selected_task(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if let Some(task_index) = self.get_task_index_from_display_position(selected) {
                let task_text = self.tasks[task_index].text.clone();
                self.tasks.remove(task_index);
                
                let commit_message = format!("🗑️ Deleted task: \"{}\"", task_text);
                self.save_tasks_with_message(Some(&commit_message));
                
                // Adjust selection
                let new_total_items = self.get_total_display_items();
                if new_total_items == 0 {
                    self.list_state.select(None);
                } else if selected >= new_total_items {
                    self.list_state.select(Some(new_total_items - 1));
                }
            }
        }
    }

    pub fn get_all_tags(&self) -> Vec<String> {
        let mut tags = std::collections::HashSet::new();
        for task in &self.tasks {
            for tag in &task.tags {
                tags.insert(tag.clone());
            }
        }
        let mut sorted_tags: Vec<String> = tags.into_iter().collect();
        sorted_tags.sort();
        sorted_tags
    }

    pub fn get_tasks_by_tag(&self, tag: &str) -> Vec<usize> {
        self.tasks
            .iter()
            .enumerate()
            .filter(|(_, task)| {
                task.tags.contains(&tag.to_string()) && (self.show_completed || !task.completed)
            })
            .map(|(i, _)| i)
            .collect()
    }

    pub fn toggle_view_mode(&mut self) {
        match self.view_mode {
            ViewMode::Tasks => {
                self.view_mode = ViewMode::TagsMenu;
                let tags = self.get_all_tags();
                if !tags.is_empty() {
                    self.tags_list_state.select(Some(0));
                }
            }
            ViewMode::TagsMenu => {
                self.view_mode = ViewMode::Tasks;
                self.selected_tag = None;
            }
        }
    }

    pub fn manual_sync(&mut self) {
        if !is_todoist_sync_enabled() {
            self.sync_status = Some("⚠ Todoist sync not enabled".to_string());
            return;
        }

        self.sync_status = Some("🔄 Syncing...".to_string());

        // Run sync in blocking manner
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                trigger_todoist_sync().await
            })
        });

        match result {
            Ok(_) => {
                self.load_tasks(); // Reload to show synced tasks
                self.sync_status = Some("✓ Synced with Todoist".to_string());
            }
            Err(e) => {
                self.sync_status = Some(format!("⚠ Sync failed: {}", e));
            }
        }
    }

    pub fn next_tag(&mut self) {
        let tags = self.get_all_tags();
        let total_items = tags.len() + 1; // +1 for "All Tasks" option
        
        let i = match self.tags_list_state.selected() {
            Some(i) => {
                if i >= total_items - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.tags_list_state.select(Some(i));
    }

    pub fn previous_tag(&mut self) {
        let tags = self.get_all_tags();
        let total_items = tags.len() + 1; // +1 for "All Tasks" option
        
        let i = match self.tags_list_state.selected() {
            Some(i) => {
                if i == 0 {
                    total_items - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.tags_list_state.select(Some(i));
    }

    pub fn select_tag(&mut self) {
        let tags = self.get_all_tags();
        if let Some(selected) = self.tags_list_state.selected() {
            if selected == 0 {
                // "All Tasks" option selected
                self.selected_tag = None;
            } else if let Some(tag) = tags.get(selected - 1) {
                // Adjust index by -1 since "All Tasks" is at index 0
                self.selected_tag = Some(tag.clone());
            }
            self.view_mode = ViewMode::Tasks;
        }
    }
}

pub fn run_tui(working_dir: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run it
    let app = App::new(working_dir);
    let res = run_app(&mut terminal, app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        // Check if splash screen should be dismissed
        if app.show_splash && app.splash_timer.elapsed().as_secs() >= 2 {
            app.show_splash = false;
        }

        terminal.draw(|f| ui(f, &mut app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                // Any key dismisses splash screen
                if app.show_splash {
                    app.show_splash = false;
                    continue;
                }

                // Any key dismisses notes popup
                if app.show_notes {
                    app.show_notes = false;
                    app.selected_task_for_notes = None;
                    continue;
                }

                match app.input_mode {
                    InputMode::Normal => match app.view_mode {
                        ViewMode::Tasks => match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Char('a') | KeyCode::Char('i') => {
                                app.input_mode = InputMode::Editing;
                            }
                            KeyCode::Char('j') | KeyCode::Down => {
                                app.next_task();
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                app.previous_task();
                            }
                            KeyCode::Char(' ') | KeyCode::Enter => {
                                app.toggle_completed();
                            }
                            KeyCode::Char('d') | KeyCode::Delete => {
                                app.delete_selected_task();
                            }
                            KeyCode::Char('c') => {
                                app.show_completed = !app.show_completed;
                            }
                            KeyCode::Char('h') | KeyCode::F(1) => {
                                app.show_help = !app.show_help;
                            }
                            KeyCode::Char('r') => {
                                app.load_tasks();
                            }
                            KeyCode::Char('n') => {
                                if let Some(selected) = app.list_state.selected() {
                                    if let Some(task_index) = app.get_task_index_from_display_position(selected) {
                                        app.selected_task_for_notes = Some(task_index);
                                        app.show_notes = true;
                                    }
                                }
                            }
                            KeyCode::Char('t') => {
                                app.toggle_view_mode();
                            }
                            KeyCode::Char('s') => {
                                // Trigger manual Todoist sync
                                app.manual_sync();
                            }
                            KeyCode::Esc => {
                                app.selected_tag = None;
                            }
                            _ => {}
                        }
                        ViewMode::TagsMenu => match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Char('j') | KeyCode::Down => {
                                app.next_tag();
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                app.previous_tag();
                            }
                            KeyCode::Enter => {
                                app.select_tag();
                            }
                            KeyCode::Char('t') | KeyCode::Esc => {
                                app.toggle_view_mode();
                            }
                            KeyCode::Char('h') | KeyCode::F(1) => {
                                app.show_help = !app.show_help;
                            }
                            _ => {}
                        }
                    }
                    InputMode::Editing => match key.code {
                        KeyCode::Enter => {
                            app.add_new_task();
                        }
                        KeyCode::Char(c) => {
                            app.input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                            app.input.clear();
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    // Splash screen
    if app.show_splash {
        draw_splash_screen(f);
        return;
    }

    // Help popup
    if app.show_help {
        draw_help_popup(f);
        return;
    }

    // Notes popup
    if app.show_notes {
        draw_notes_popup(f, app);
        return;
    }

    // Main layout
    match app.view_mode {
        ViewMode::Tasks => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),    // Task list
                    Constraint::Length(3), // Input
                    Constraint::Length(1), // Status line
                ])
                .split(f.size());

            draw_task_list(f, app, chunks[0]);
            draw_input(f, app, chunks[1]);
            draw_status_line(f, app, chunks[2]);
        }
        ViewMode::TagsMenu => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),    // Tags list
                    Constraint::Length(1), // Status line
                ])
                .split(f.size());

            draw_tags_menu(f, app, chunks[0]);
            draw_status_line(f, app, chunks[1]);
        }
    }
}

fn draw_task_list(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let grouped_tasks = if let Some(tag) = &app.selected_tag {
        // Show tasks filtered by tag
        let task_indices = app.get_tasks_by_tag(tag);
        if task_indices.is_empty() {
            vec![]
        } else {
            vec![(format!("TAG: #{}", tag), task_indices)]
        }
    } else {
        app.get_grouped_tasks()
    };
    
    let mut items: Vec<ListItem> = Vec::new();
    
    // Add section headers and tasks
    for (section_name, task_indices) in grouped_tasks {
        if !task_indices.is_empty() {
            // Add section header
            items.push(ListItem::new(Line::from(vec![
                Span::styled(
                    format!("━━━ {} ━━━", section_name),
                    Style::default().fg(Color::Rgb(255, 107, 138)).add_modifier(Modifier::BOLD)
                )
            ])));
            
            // Add tasks in this section
            for &i in &task_indices {
                let task = &app.tasks[i];
                let mut spans = Vec::new();
                
                // Indentation for subtasks
                let indent = "";
                if !indent.is_empty() {
                    spans.push(Span::styled(indent, Style::default()));
                }
                
                // Checkbox
                let checkbox = if task.completed { "☑" } else { "☐" };
                spans.push(Span::styled(
                    format!("{} ", checkbox),
                    if task.completed {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default().fg(Color::Rgb(255, 107, 138))
                    }
                ));

                // Task text with ID
                let text_style = if task.completed {
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::CROSSED_OUT)
                } else {
                    Style::default().fg(Color::White)
                };
                let id_display = if task.id.len() > 8 { &task.id[..8] } else { &task.id };
                spans.push(Span::styled(
                    format!("{} [{}]", task.text, id_display), 
                    text_style
                ));

                // Deadline indicator
                if let Some(deadline) = task.deadline {
                    let today = chrono::Local::now().date_naive();
                    let (indicator, color) = if deadline < today {
                        (" ⚠️ OVERDUE", Color::Red)
                    } else if deadline == today {
                        (" 🔴 DUE TODAY", Color::Rgb(255, 107, 138))
                    } else {
                        (" 📅", Color::Rgb(255, 107, 138))
                    };
                    
                    spans.push(Span::styled(
                        format!("{} {}", indicator, deadline.format("%m/%d")),
                        Style::default().fg(color)
                    ));
                }

                // Tags
                for tag in &task.tags {
                    spans.push(Span::styled(
                        format!(" 🏷️#{}", tag),
                        Style::default().fg(Color::Green)
                    ));
                }

                // Reminder
                if let Some(reminder) = task.reminder {
                    spans.push(Span::styled(
                        format!(" 🔔{}", reminder.format("%m/%d")),
                        Style::default().fg(Color::Rgb(255, 107, 138))
                    ));
                }

                // Notes - displayed last like a comment
                if let Some(ref notes) = task.notes {
                    spans.push(Span::styled(
                        format!(" //{}", notes),
                        Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)
                    ));
                }

                // Importance
                if let Some(importance) = task.importance {
                    let (color, style) = match importance {
                        1 => (Color::Red, Modifier::BOLD),      // Very important
                        2 => (Color::LightRed, Modifier::empty()), // High importance
                        3 => (Color::Yellow, Modifier::empty()),   // Medium importance
                        4 => (Color::LightBlue, Modifier::empty()), // Low importance
                        5 => (Color::DarkGray, Modifier::empty()),  // Not important
                        _ => (Color::White, Modifier::empty()),
                    };
                    spans.push(Span::styled(
                        format!(" ⭐${}", importance),
                        Style::default().fg(color).add_modifier(style)
                    ));
                }

                items.push(ListItem::new(Line::from(spans)));
            }
            
            // Add spacing between sections
            items.push(ListItem::new(Line::from("")));
        }
    }

    let title = if let Some(tag) = &app.selected_tag {
        if app.show_completed {
            format!("Tasks: #{} (All)", tag)
        } else {
            format!("Tasks: #{} (Active)", tag)
        }
    } else if app.show_completed {
        "Tasks (All)".to_string()
    } else {
        "Tasks (Active)".to_string()
    };

    let tasks_list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(Color::Rgb(255, 107, 138))))
        .highlight_style(Style::default().bg(Color::Black).fg(Color::Rgb(255, 107, 138)))
        .highlight_symbol("► ");

    f.render_stateful_widget(tasks_list, area, &mut app.list_state);
}

fn draw_tags_menu(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let tags = app.get_all_tags();
    let mut items: Vec<ListItem> = Vec::new();

    // Add "All Tasks" option
    items.push(ListItem::new(Line::from(vec![
        Span::styled("📋 All Tasks", Style::default().fg(Color::White))
    ])));

    // Add individual tags
    for tag in &tags {
        let task_count = app.get_tasks_by_tag(tag).len();
        items.push(ListItem::new(Line::from(vec![
            Span::styled("🏷️  #", Style::default().fg(Color::Green)),
            Span::styled(tag, Style::default().fg(Color::Green)),
            Span::styled(format!(" ({})", task_count), Style::default().fg(Color::DarkGray)),
        ])));
    }

    if tags.is_empty() {
        items.push(ListItem::new(Line::from(vec![
            Span::styled("No tags found", Style::default().fg(Color::DarkGray))
        ])));
    }

    let tags_list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .title("Tags Menu")
            .border_style(Style::default().fg(Color::Rgb(255, 107, 138))))
        .highlight_style(Style::default().bg(Color::Black).fg(Color::Rgb(255, 107, 138)))
        .highlight_symbol("► ");

    f.render_stateful_widget(tags_list, area, &mut app.tags_list_state);
}

fn draw_input(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let input = Paragraph::new(app.input.as_str())
        .style(match app.input_mode {
            InputMode::Normal => Style::default().fg(Color::White),
            InputMode::Editing => Style::default().fg(Color::Rgb(255, 107, 138)),
        })
        .block(Block::default()
            .borders(Borders::ALL)
            .title("Add Task")
            .border_style(Style::default().fg(Color::Rgb(255, 107, 138))))
        .wrap(Wrap { trim: true });
    
    f.render_widget(input, area);

    if app.input_mode == InputMode::Editing {
        f.set_cursor(
            area.x + app.input.len() as u16 + 1,
            area.y + 1,
        );
    }
}

fn draw_status_line(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let visible_count = app.get_visible_tasks().len();
    let total_count = app.tasks.len();
    let completed_count = app.tasks.iter().filter(|t| t.completed).count();

    let mode_text = match app.input_mode {
        InputMode::Normal => "NORMAL",
        InputMode::Editing => "EDITING",
    };

    let view_info = match app.view_mode {
        ViewMode::Tasks => {
            if let Some(tag) = &app.selected_tag {
                format!("Filtered by #{}", tag)
            } else {
                "All tasks".to_string()
            }
        }
        ViewMode::TagsMenu => "Tags menu".to_string(),
    };

    let sync_info = if let Some(ref sync_status) = app.sync_status {
        format!(" | {}", sync_status)
    } else {
        String::new()
    };

    let status_text = format!(
        "{} | {} | Tasks: {} active, {} completed, {} total{} | s:sync t:tags h:help q:quit",
        mode_text, view_info, visible_count, completed_count, total_count, sync_info
    );

    let status = Paragraph::new(status_text)
        .style(Style::default().fg(Color::White).bg(Color::Black));

    f.render_widget(status, area);
}

fn draw_splash_screen(f: &mut Frame) {
    let splash_art = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("    ██    ██  █████  ██████  ███    ███ ████████ ██      ", Style::default().fg(Color::Rgb(255, 107, 138))),
        ]),
        Line::from(vec![
            Span::styled("     ██  ██  ██   ██ ██   ██ ████  ████    ██    ██      ", Style::default().fg(Color::Rgb(255, 107, 138))),
        ]),
        Line::from(vec![
            Span::styled("      ████   ███████ ██████  ██ ████ ██    ██    ██      ", Style::default().fg(Color::Rgb(255, 107, 138))),
        ]),
        Line::from(vec![
            Span::styled("       ██    ██   ██ ██   ██ ██  ██  ██    ██    ██      ", Style::default().fg(Color::Rgb(255, 107, 138))),
        ]),
        Line::from(vec![
            Span::styled("       ██    ██   ██ ██   ██ ██      ██    ██    ███████ ", Style::default().fg(Color::Green)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("              Yet Another Rust Markdown Todo List", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("    ┌─────────────────────────────────────────────────────────────┐", Style::default().fg(Color::Rgb(255, 107, 138))),
        ]),
        Line::from(vec![
            Span::styled("    │  📝 ", Style::default().fg(Color::Rgb(255, 107, 138))),
            Span::styled("Organize your tasks with deadlines and projects    ", Style::default().fg(Color::White)),
            Span::styled("│", Style::default().fg(Color::Rgb(255, 107, 138))),
        ]),
        Line::from(vec![
            Span::styled("    │  ⚡ ", Style::default().fg(Color::Rgb(255, 107, 138))),
            Span::styled("Fast, lightweight, and markdown-based             ", Style::default().fg(Color::White)),
            Span::styled("│", Style::default().fg(Color::Rgb(255, 107, 138))),
        ]),
        Line::from(vec![
            Span::styled("    │  🎯 ", Style::default().fg(Color::Rgb(255, 107, 138))),
            Span::styled("Visual deadline tracking and email reminders      ", Style::default().fg(Color::White)),
            Span::styled("│", Style::default().fg(Color::Rgb(255, 107, 138))),
        ]),
        Line::from(vec![
            Span::styled("    └─────────────────────────────────────────────────────────────┘", Style::default().fg(Color::Rgb(255, 107, 138))),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("                    Press any key to continue...", Style::default().fg(Color::Rgb(255, 107, 138)).add_modifier(Modifier::ITALIC)),
        ]),
    ];

    let splash_paragraph = Paragraph::new(splash_art)
        .block(Block::default())
        .wrap(Wrap { trim: true });

    let area = centered_rect(90, 90, f.size());
    f.render_widget(Clear, area);
    f.render_widget(splash_paragraph, area);
}

fn draw_notes_popup(f: &mut Frame, app: &App) {
    if let Some(task_index) = app.selected_task_for_notes {
        if let Some(task) = app.tasks.get(task_index) {
            let popup_area = centered_rect(60, 50, f.size());
            
            f.render_widget(Clear, popup_area);
            
            let notes_text = if let Some(ref notes) = task.notes {
                notes.clone()
            } else {
                "No notes for this task.".to_string()
            };
            
            let notes_lines = vec![
                Line::from(vec![
                    Span::styled("Task: ", Style::default().fg(Color::Rgb(255, 107, 138))),
                    Span::styled(&task.text, Style::default().fg(Color::White)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Notes:", Style::default().fg(Color::Rgb(255, 107, 138))),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled(notes_text, Style::default().fg(Color::White)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Press any key to close", Style::default().fg(Color::DarkGray)),
                ]),
            ];

            let notes_paragraph = Paragraph::new(notes_lines)
                .block(Block::default()
                    .title("Task Notes")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Rgb(255, 107, 138))))
                .wrap(Wrap { trim: true });

            f.render_widget(notes_paragraph, popup_area);
        }
    }
}

fn draw_help_popup(f: &mut Frame) {
    let popup_area = centered_rect(70, 80, f.size());
    
    f.render_widget(Clear, popup_area);
    
    let help_text = vec![
        Line::from("YARMTL - Help"),
        Line::from(""),
        Line::from("Navigation:"),
        Line::from("  j/↓    - Next task"),
        Line::from("  k/↑    - Previous task"),
        Line::from("  Enter  - Toggle task completion"),
        Line::from("  Space  - Toggle task completion"),
        Line::from(""),
        Line::from("Task Management:"),
        Line::from("  a/i    - Add new task"),
        Line::from("  d/Del  - Delete selected task"),
        Line::from("  c      - Toggle show completed tasks"),
        Line::from("  r      - Reload tasks from file"),
        Line::from("  n      - View task notes"),
        Line::from("  s      - Sync with Todoist"),
        Line::from("  t      - Toggle tags menu"),
        Line::from("  Esc    - Clear tag filter"),
        Line::from(""),
        Line::from("Task Syntax:"),
        Line::from("  !2025-10-01    - Set deadline"),
        Line::from("  @today         - Set reminder for today"),
        Line::from("  @tomorrow      - Set reminder for tomorrow"),
        Line::from("  @2025-10-01    - Set reminder for date"),
        Line::from("  #work #urgent  - Add multiple tags"),
        Line::from("  //note text    - Add task notes"),
        Line::from("  $1             - Set importance (1=very important, 5=not important)"),
        Line::from(""),
        Line::from("Examples:"),
        Line::from("  \"Finish report !2025-10-01 @today #work #urgent $1 //Important meeting\""),
        Line::from("  \"Review docs #work $3 //Check formatting\""),
        Line::from(""),
        Line::from("Other:"),
        Line::from("  h/F1   - Toggle this help"),
        Line::from("  q      - Quit"),
        Line::from(""),
        Line::from("Press any key to close help"),
    ];

    let help_paragraph = Paragraph::new(help_text)
        .block(Block::default()
            .title("Help")
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Black)))
        .wrap(Wrap { trim: true });

    f.render_widget(help_paragraph, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: ratatui::layout::Rect) -> ratatui::layout::Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}