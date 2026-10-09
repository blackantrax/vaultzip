#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

mod launch;
mod shell;

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use eframe::egui::{self, Color32, RichText};
use launch::{Launch, Mode};
use vaultzip_core::{self as core, Progress, Strength};

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Create,
    Extract,
}

/// Where an opened archive is extracted by default.
#[derive(PartialEq, Clone, Copy)]
enum DestMode {
    /// A new folder named after the archive.
    Folder,
    /// The folder that contains the archive.
    Here,
}

enum Msg {
    Done(String),
    Cancelled,
    Failed(String),
}

struct App {
    tab: Tab,
    // create
    inputs: Vec<PathBuf>,
    output: String,
    encrypt: bool,
    password: String,
    confirm: String,
    show_password: bool,
    // extract
    archive: Option<PathBuf>,
    entries: Vec<core::Entry>,
    needs_password: bool,
    extract_password: String,
    dest: String,
    // batch extraction started from Explorer
    auto: Option<DestMode>,
    pending: VecDeque<PathBuf>,
    auto_start: bool,
    batch_done: usize,
    // shared
    status: Option<(bool, String)>,
    busy: bool,
    rx: Option<Receiver<Msg>>,
    progress: Arc<Progress>,
    shell_status: Option<String>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            tab: Tab::Create,
            inputs: Vec::new(),
            output: String::new(),
            encrypt: true,
            password: String::new(),
            confirm: String::new(),
            show_password: false,
            archive: None,
            entries: Vec::new(),
            needs_password: false,
            extract_password: String::new(),
            dest: String::new(),
            auto: None,
            pending: VecDeque::new(),
            auto_start: false,
            batch_done: 0,
            status: None,
            busy: false,
            rx: None,
            progress: Arc::new(Progress::new()),
            shell_status: None,
        }
    }
}

fn strength_color(s: Strength) -> Color32 {
    match s {
        Strength::VeryWeak => Color32::from_rgb(200, 50, 50),
        Strength::Weak => Color32::from_rgb(220, 120, 40),
        Strength::Fair => Color32::from_rgb(210, 180, 40),
        Strength::Strong => Color32::from_rgb(90, 170, 70),
        Strength::VeryStrong => Color32::from_rgb(40, 150, 90),
    }
}

fn human_size(n: u64) -> String {
    const U: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", U[i])
    }
}

impl App {
    /// Build the starting state from how the program was launched.
    fn from_launch(launch: Launch) -> Self {
        let mut app = App::default();
        match launch.mode {
            Mode::Add => {
                app.tab = Tab::Create;
                app.encrypt = launch.encrypt;
                for p in launch.paths {
                    app.add_input(p);
                }
            }
            Mode::Open => {
                app.tab = Tab::Extract;
                if let Some(p) = launch.paths.into_iter().next() {
                    app.load_archive(p, DestMode::Folder);
                }
            }
            Mode::ExtractHere | Mode::ExtractFolder => {
                app.tab = Tab::Extract;
                app.auto = Some(if launch.mode == Mode::ExtractHere {
                    DestMode::Here
                } else {
                    DestMode::Folder
                });
                app.pending = launch.paths.into();
                app.advance_pending();
            }
            _ => {}
        }
        app
    }

    fn handle_drops(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if dropped.is_empty() || self.busy {
            return;
        }
        match self.tab {
            Tab::Create => {
                for p in dropped {
                    self.add_input(p);
                }
            }
            Tab::Extract => {
                if let Some(p) = dropped.into_iter().next() {
                    self.auto = None;
                    self.pending.clear();
                    self.load_archive(p, DestMode::Folder);
                }
            }
        }
    }

    fn add_input(&mut self, p: PathBuf) {
        if !self.inputs.contains(&p) {
            self.inputs.push(p);
        }
        if self.output.is_empty() {
            if let Some(first) = self.inputs.first() {
                let stem = first
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("archive");
                let dir = first.parent().unwrap_or_else(|| Path::new("."));
                self.output = dir.join(format!("{stem}.zip")).display().to_string();
            }
        }
    }

    fn load_archive(&mut self, p: PathBuf, mode: DestMode) {
        self.status = None;
        match core::list_archive(&p) {
            Ok(entries) => {
                self.needs_password = entries.iter().any(|e| e.encrypted);
                self.entries = entries;
                let stem = p
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("extracted");
                let dir = p.parent().unwrap_or_else(|| Path::new("."));
                self.dest = match mode {
                    DestMode::Folder => dir.join(stem).display().to_string(),
                    DestMode::Here => dir.display().to_string(),
                };
                self.archive = Some(p);
            }
            Err(e) => {
                self.archive = None;
                self.entries.clear();
                self.status = Some((false, format!("Cannot open archive: {e}")));
            }
        }
    }

    /// Load the next queued archive and start it automatically when no
    /// password has to be typed first.
    fn advance_pending(&mut self) -> bool {
        let (Some(mode), Some(next)) = (self.auto, self.pending.pop_front()) else {
            return false;
        };
        self.load_archive(next, mode);
        if self.archive.is_some() {
            self.auto_start = !self.needs_password || !self.extract_password.is_empty();
            true
        } else {
            false
        }
    }

    fn start_create(&mut self, ctx: &egui::Context) {
        let inputs = self.inputs.clone();
        let output = PathBuf::from(self.output.trim());
        let pw = self.encrypt.then(|| self.password.clone());
        self.spawn(ctx, move |progress| {
            core::create_archive_with_progress(&inputs, &output, pw.as_deref(), progress)
                .map(|_| format!("Created {}", output.display()))
        });
    }

    fn start_extract(&mut self, ctx: &egui::Context) {
        let Some(archive) = self.archive.clone() else {
            return;
        };
        let dest = PathBuf::from(self.dest.trim());
        let pw = self.needs_password.then(|| self.extract_password.clone());
        self.spawn(ctx, move |progress| {
            core::extract_archive_with_progress(&archive, &dest, pw.as_deref(), progress)
                .map(|_| format!("Extracted to {}", dest.display()))
        });
    }

    fn spawn<F>(&mut self, ctx: &egui::Context, job: F)
    where
        F: FnOnce(&Progress) -> core::Result<String> + Send + 'static,
    {
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        let progress = Arc::clone(&self.progress);
        progress.reset();
        self.busy = true;
        self.status = None;
        self.rx = Some(rx);
        thread::spawn(move || {
            let msg = match job(&progress) {
                Ok(m) => Msg::Done(m),
                Err(core::Error::Cancelled) => Msg::Cancelled,
                Err(e) => Msg::Failed(e.to_string()),
            };
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
    }

    fn poll(&mut self) {
        let msg = match &self.rx {
            Some(rx) => match rx.try_recv() {
                Ok(m) => m,
                Err(_) => return,
            },
            None => return,
        };
        self.busy = false;
        self.rx = None;
        match msg {
            Msg::Done(m) => {
                if self.auto.is_some() {
                    self.batch_done += 1;
                    if !self.pending.is_empty() {
                        if self.advance_pending() {
                            self.status = Some((
                                true,
                                format!("Extracted {} archive(s), continuing...", self.batch_done),
                            ));
                        }
                        return;
                    }
                    if self.batch_done > 1 {
                        self.status =
                            Some((true, format!("Extracted {} archives", self.batch_done)));
                        return;
                    }
                }
                self.status = Some((true, m));
            }
            Msg::Cancelled => {
                self.pending.clear();
                self.status = Some((false, "Cancelled. No partial archive was kept.".into()));
            }
            Msg::Failed(e) => {
                self.pending.clear();
                self.status = Some((false, e));
            }
        }
    }

    fn create_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.add_space(6.0);
        drop_zone(ui, "Drop files or folders here", !self.inputs.is_empty());
        ui.horizontal(|ui| {
            if ui.button("Add files").clicked() {
                if let Some(files) = rfd::FileDialog::new().pick_files() {
                    for f in files {
                        self.add_input(f);
                    }
                }
            }
            if ui.button("Add folder").clicked() {
                if let Some(d) = rfd::FileDialog::new().pick_folder() {
                    self.add_input(d);
                }
            }
            if !self.inputs.is_empty() && ui.button("Clear").clicked() {
                self.inputs.clear();
            }
        });

        let mut remove = None;
        egui::ScrollArea::vertical()
            .max_height(110.0)
            .show(ui, |ui| {
                for (i, p) in self.inputs.iter().enumerate() {
                    ui.horizontal(|ui| {
                        if ui.small_button("x").clicked() {
                            remove = Some(i);
                        }
                        ui.label(p.display().to_string());
                    });
                }
            });
        if let Some(i) = remove {
            self.inputs.remove(i);
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Save as");
            ui.add(egui::TextEdit::singleline(&mut self.output).desired_width(380.0));
            if ui.button("Browse").clicked() {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter("ZIP archive", &["zip"])
                    .save_file()
                {
                    self.output = p.display().to_string();
                }
            }
        });

        ui.add_space(6.0);
        ui.checkbox(&mut self.encrypt, "Protect with a password (AES-256)");
        let mut password_ok = true;
        if self.encrypt {
            ui.horizontal(|ui| {
                ui.label("Password");
                ui.add(
                    egui::TextEdit::singleline(&mut self.password)
                        .password(!self.show_password)
                        .desired_width(260.0),
                );
                ui.checkbox(&mut self.show_password, "Show");
            });
            ui.horizontal(|ui| {
                ui.label("Confirm   ");
                ui.add(
                    egui::TextEdit::singleline(&mut self.confirm)
                        .password(!self.show_password)
                        .desired_width(260.0),
                );
            });
            if !self.password.is_empty() {
                let s = core::password_strength(&self.password);
                ui.label(
                    RichText::new(format!("Strength: {}", s.label())).color(strength_color(s)),
                );
            }
            if self.password.is_empty() {
                password_ok = false;
            } else if self.password != self.confirm {
                password_ok = false;
                ui.colored_label(Color32::from_rgb(200, 50, 50), "Passwords do not match");
            }
            ui.label(RichText::new("A lost password cannot be recovered.").weak());
        }

        ui.add_space(8.0);
        let ready =
            !self.inputs.is_empty() && !self.output.trim().is_empty() && password_ok && !self.busy;
        if ui
            .add_enabled(ready, egui::Button::new("Create archive"))
            .clicked()
        {
            self.start_create(ctx);
        }
    }

    fn extract_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.add_space(6.0);
        drop_zone(ui, "Drop a ZIP archive here", self.archive.is_some());
        if ui
            .add_enabled(!self.busy, egui::Button::new("Open archive"))
            .clicked()
        {
            if let Some(p) = rfd::FileDialog::new()
                .add_filter("ZIP archive", &["zip"])
                .pick_file()
            {
                self.auto = None;
                self.pending.clear();
                self.load_archive(p, DestMode::Folder);
            }
        }
        if let Some(a) = &self.archive {
            ui.label(RichText::new(a.display().to_string()).strong());
            egui::ScrollArea::vertical()
                .max_height(150.0)
                .show(ui, |ui| {
                    for e in &self.entries {
                        ui.label(format!(
                            "{}{}  ({})",
                            if e.encrypted { "[locked] " } else { "" },
                            e.name,
                            human_size(e.size)
                        ));
                    }
                });
            ui.add_space(6.0);
            if self.needs_password {
                ui.horizontal(|ui| {
                    ui.label("Password");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.extract_password)
                            .password(true)
                            .desired_width(260.0),
                    );
                });
            }
            ui.horizontal(|ui| {
                ui.label("Extract to");
                ui.add(egui::TextEdit::singleline(&mut self.dest).desired_width(360.0));
                if ui.button("Browse").clicked() {
                    if let Some(d) = rfd::FileDialog::new().pick_folder() {
                        self.dest = d.display().to_string();
                    }
                }
            });
            ui.add_space(8.0);
            let ready = !self.dest.trim().is_empty()
                && (!self.needs_password || !self.extract_password.is_empty())
                && !self.busy;
            if ui
                .add_enabled(ready, egui::Button::new("Extract"))
                .clicked()
            {
                self.start_extract(ctx);
            }
        }
    }

    fn progress_ui(&self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let p = &self.progress;
        if p.total() == 0 {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Preparing...");
            });
        } else {
            ui.add(
                egui::ProgressBar::new(p.fraction())
                    .show_percentage()
                    .desired_width(ui.available_width().min(480.0)),
            );
            ui.label(
                RichText::new(format!(
                    "{} of {}   {}",
                    human_size(p.done()),
                    human_size(p.total()),
                    p.current()
                ))
                .weak(),
            );
        }
        if ui.button("Cancel").clicked() {
            p.cancel();
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn settings_ui(&mut self, ui: &mut egui::Ui) {
        if !shell::supported() {
            return;
        }
        ui.add_space(6.0);
        ui.collapsing("Settings", |ui| {
            ui.label("Windows Explorer right-click menu");
            ui.horizontal(|ui| {
                let installed = shell::is_installed();
                if ui
                    .button(if installed { "Repair menu" } else { "Add menu" })
                    .clicked()
                {
                    self.shell_status = Some(match shell::install() {
                        Ok(()) => "Menu added. Right-click a file, folder or ZIP.".to_string(),
                        Err(e) => e,
                    });
                }
                if installed && ui.button("Remove menu").clicked() {
                    self.shell_status = Some(match shell::uninstall() {
                        Ok(()) => "Menu removed.".to_string(),
                        Err(e) => e,
                    });
                }
            });
            ui.label(RichText::new("On Windows 11, look under Show more options.").weak());
            if let Some(s) = &self.shell_status {
                ui.label(s);
            }
        });
    }
}

fn drop_zone(ui: &mut egui::Ui, text: &str, filled: bool) {
    let hovering = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
    let stroke_color = if hovering {
        Color32::from_rgb(60, 130, 220)
    } else {
        ui.visuals()
            .widgets
            .noninteractive
            .fg_stroke
            .color
            .gamma_multiply(0.5)
    };
    let height = if filled { 40.0 } else { 70.0 };
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    ui.painter()
        .rect_stroke(rect, 8.0, egui::Stroke::new(1.5_f32, stroke_color));
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(15.0),
        ui.visuals().text_color(),
    );
    ui.add_space(4.0);
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        self.handle_drops(ctx);
        if self.auto_start && !self.busy && self.archive.is_some() {
            self.auto_start = false;
            self.start_extract(ctx);
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("VaultZip");
                ui.label(RichText::new("Free archiver with AES-256 protection").weak());
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.add_enabled_ui(!self.busy, |ui| {
                    ui.selectable_value(&mut self.tab, Tab::Create, "Create");
                    ui.selectable_value(&mut self.tab, Tab::Extract, "Extract");
                });
            });
            ui.separator();
            match self.tab {
                Tab::Create => self.create_ui(ui, ctx),
                Tab::Extract => self.extract_ui(ui, ctx),
            }
            ui.add_space(8.0);
            if self.busy {
                self.progress_ui(ui, ctx);
            }
            if let Some((ok, msg)) = &self.status {
                let color = if *ok {
                    Color32::from_rgb(40, 150, 90)
                } else {
                    Color32::from_rgb(200, 50, 50)
                };
                ui.colored_label(color, msg);
            }
            self.settings_ui(ui);
        });
    }
}

fn notify(text: &str) {
    let _ = rfd::MessageDialog::new()
        .set_title("VaultZip")
        .set_description(text)
        .show();
}

/// Show the outcome of a menu change. Failures always end the process with
/// exit code 1 so installers and scripts can detect them.
fn report(result: Result<(), String>, ok_text: &str, silent: bool) {
    match result {
        Ok(()) => {
            if !silent {
                notify(ok_text);
            }
        }
        Err(e) => {
            if silent {
                eprintln!("{e}");
            } else {
                notify(&e);
            }
            std::process::exit(1);
        }
    }
}

fn main() -> eframe::Result<()> {
    let mut launch = match launch::parse(std::env::args().skip(1)) {
        Ok(l) => l,
        Err(e) => {
            notify(&format!("{e}\n\nUsage: vaultzip-gui [--add [--encrypt] | --open | --extract-here | --extract-folder] FILES"));
            return Ok(());
        }
    };

    match launch.mode {
        Mode::InstallShell => {
            report(
                shell::install(),
                "The VaultZip right-click menu was added.",
                launch.silent,
            );
            return Ok(());
        }
        Mode::UninstallShell => {
            report(
                shell::uninstall(),
                "The VaultZip right-click menu was removed.",
                launch.silent,
            );
            return Ok(());
        }
        _ => {}
    }

    // Explorer starts one process per selected item. Merge them into one window.
    // Windows only: the temp folder there is per-user, while /tmp on other
    // systems is shared and another user could inject paths into the queue.
    if cfg!(windows) && launch.mode != Mode::Normal && launch.paths.len() == 1 {
        let dir = std::env::temp_dir()
            .join("vaultzip-queue")
            .join(launch::queue_key(&launch));
        match launch::coalesce(&dir, &launch.paths, Duration::from_millis(600)) {
            Ok(Some(all)) => launch.paths = all,
            Ok(None) => return Ok(()),
            Err(_) => {}
        }
    }

    let icon = egui::IconData {
        rgba: include_bytes!("../../../assets/icon-64.rgba").to_vec(),
        width: 64,
        height: 64,
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([640.0, 600.0])
            .with_min_inner_size([560.0, 500.0])
            .with_icon(icon)
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "VaultZip",
        options,
        Box::new(move |_cc| Box::new(App::from_launch(launch))),
    )
}
