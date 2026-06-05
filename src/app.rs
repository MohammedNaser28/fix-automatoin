use crate::sys::blkdev::DiskInfo;
use ratatui::widgets::TableState;
use std::sync::mpsc::Receiver;

// ─── Scan State ────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ScanState {
    Scanning,
    Done,
}

// ─── Screens ──────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum CurrentScreen {
    Welcome,
    SelectRoot,
    SelectEfi,
    Confirm,
    ActionMenu,
    DiagnoseLog,
    ExecLog,
    Result,
    LogExport,
}

// ─── Confirm focus ────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ConfirmFocus {
    Confirm,
    Back,
}

// ─── Actions ──────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Action {
    FixGrub,
    FixFstab,
    FixGrubAndFstab,
    OpenChrootShell,
    // Post-MVP items (shown grayed)
    RestoreWindowsEfi,
    PartitionManager,
    ExportLogs,
    DiagnoseWithAI,
}

impl Action {
    pub fn is_available(self) -> bool {
        matches!(
            self,
            Action::FixGrub | Action::FixFstab | Action::FixGrubAndFstab | Action::OpenChrootShell
        )
    }
    pub fn label(self) -> &'static str {
        match self {
            Action::FixGrub => "fix grub",
            Action::FixFstab => "fix fstab",
            Action::FixGrubAndFstab => "fix grub + fstab",
            Action::OpenChrootShell => "open chroot shell",
            Action::RestoreWindowsEfi => "restore windows EFI",
            Action::PartitionManager => "partition manager",
            Action::ExportLogs => "export logs",
            Action::DiagnoseWithAI => "diagnose with AI",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Action::FixGrub => "reinstall + regenerate grub.cfg",
            Action::FixFstab => "auto-regen or edit manually",
            Action::FixGrubAndFstab => "recommended",
            Action::OpenChrootShell => "drop into chroot shell",
            Action::RestoreWindowsEfi => "recover from NTFS backup",
            Action::PartitionManager => "create - delete - resize",
            Action::ExportLogs => "QR code - paste URL",
            Action::DiagnoseWithAI => "send logs to claude",
        }
    }
}

/// Flat list of action menu items. `None` = section header.
/// Index positions are stable so `action_cursor` can reference them directly.
pub const ACTION_ITEMS: &[Option<Action>] = &[
    None, // "repair" header
    Some(Action::FixGrub),
    Some(Action::FixFstab),
    Some(Action::FixGrubAndFstab),
    Some(Action::RestoreWindowsEfi),
    None, // "disk" header
    Some(Action::PartitionManager),
    None, // "help" header
    Some(Action::ExportLogs),
    Some(Action::DiagnoseWithAI),
    Some(Action::OpenChrootShell),
];

// ─── Execution log ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum LogKind {
    Step,   // > gray  - step header
    Output, // white  - command stdout/stderr
    Ok,     // ok green
    Warn,   // !! yellow
    Error,  // !! red
    DiagnosisResult(Vec<String>, Option<Action>),
    Done, // internal signal - repair finished
}

#[derive(Debug, Clone)]
pub struct LogLine {
    pub kind: LogKind,
    pub text: String,
}

impl LogLine {
    pub fn step(t: impl Into<String>) -> Self {
        Self {
            kind: LogKind::Step,
            text: t.into(),
        }
    }
    #[allow(dead_code)]
    pub fn output(t: impl Into<String>) -> Self {
        Self {
            kind: LogKind::Output,
            text: t.into(),
        }
    }
    pub fn ok(t: impl Into<String>) -> Self {
        Self {
            kind: LogKind::Ok,
            text: t.into(),
        }
    }
    pub fn warn(t: impl Into<String>) -> Self {
        Self {
            kind: LogKind::Warn,
            text: t.into(),
        }
    }
    pub fn error(t: impl Into<String>) -> Self {
        Self {
            kind: LogKind::Error,
            text: t.into(),
        }
    }
    pub fn done() -> Self {
        Self {
            kind: LogKind::Done,
            text: String::new(),
        }
    }
}

// ─── App ─────────────────────────────────────────────────────────────────────

pub struct App {
    pub current_screen: CurrentScreen,
    pub should_quit: bool,
    pub is_uefi: bool,
    pub detected_distro: Option<String>,
    pub confirm_focus: ConfirmFocus,
    pub network_info: Option<String>,

    // System data
    pub disks: Vec<DiskInfo>,
    pub selected_root: Option<DiskInfo>,
    pub selected_efi: Option<DiskInfo>,

    // Scan state (disk detection runs async to show immediate UI)
    pub scan_state: ScanState,
    pub scan_rx: Option<Receiver<Vec<DiskInfo>>>,

    // Shared table/list UI state
    pub table_state: TableState,

    // Action menu
    pub action_cursor: usize,
    pub selected_action: Option<Action>,
    pub diagnosis_summary: Vec<String>,
    pub recommended_action: Option<Action>,

    // Execution log
    pub log_lines: Vec<LogLine>,
    pub exec_step: usize,
    pub exec_total: usize,
    pub exec_done: bool,
    pub log_rx: Option<Receiver<LogLine>>,

    // Result & Export
    pub result_cursor: usize,
    #[allow(dead_code)]
    pub export_cursor: usize,
}

impl App {
    pub fn new() -> Self {
        let (scan_tx, scan_rx) = std::sync::mpsc::channel::<Vec<DiskInfo>>();

        std::thread::spawn(move || {
            let disks = crate::sys::blkdev::get_disks();
            let _ = scan_tx.send(disks);
        });

        let is_uefi = crate::sys::firmware::is_uefi();
        let network_info = crate::sys::network::get_ip();

        // Position cursor at the first real action (skip the first header)
        let first = ACTION_ITEMS.iter().position(|i| i.is_some()).unwrap_or(0);

        let mut table_state = TableState::default();
        table_state.select(Some(0));

        Self {
            current_screen: CurrentScreen::Welcome,
            should_quit: false,
            is_uefi,
            detected_distro: None,
            confirm_focus: ConfirmFocus::Confirm,
            network_info,

            disks: Vec::new(),
            selected_root: None,
            selected_efi: None,

            scan_state: ScanState::Scanning,
            scan_rx: Some(scan_rx),

            table_state,

            action_cursor: first,
            selected_action: None,
            diagnosis_summary: Vec::new(),
            recommended_action: None,

            log_lines: Vec::new(),
            exec_step: 0,
            exec_total: 7,
            exec_done: false,
            log_rx: None,

            result_cursor: 0,
            export_cursor: 0,
        }
    }

    // ── Partition list navigation ─────────────────────────────────────────────

    pub fn select_next(&mut self) {
        if self.disks.is_empty() {
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.disks.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    pub fn select_previous(&mut self) {
        if self.disks.is_empty() {
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.disks.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    // ── Confirm screen ────────────────────────────────────────────────────────

    pub fn toggle_confirm_buttons(&mut self) {
        self.confirm_focus = match self.confirm_focus {
            ConfirmFocus::Confirm => ConfirmFocus::Back,
            ConfirmFocus::Back => ConfirmFocus::Confirm,
        };
    }

    // ── Action menu navigation — skips section headers ────────────────────────

    pub fn action_next(&mut self) {
        let mut c = self.action_cursor + 1;
        while c < ACTION_ITEMS.len() {
            if ACTION_ITEMS[c].is_some() {
                self.action_cursor = c;
                return;
            }
            c += 1;
        }
        // Wrap to top
        if let Some(first) = ACTION_ITEMS.iter().position(|i| i.is_some()) {
            self.action_cursor = first;
        }
    }

    pub fn action_prev(&mut self) {
        if self.action_cursor == 0 {
            return;
        }
        let mut c = self.action_cursor - 1;
        loop {
            if ACTION_ITEMS[c].is_some() {
                self.action_cursor = c;
                return;
            }
            if c == 0 {
                break;
            }
            c -= 1;
        }
    }

    // ── Distro heuristic (for Confirm screen before real mount) ───────────────

    pub fn heuristic_distro(&self) -> String {
        if let Some(ref d) = self.detected_distro {
            return d.clone();
        }
        if let Some(ref root) = self.selected_root {
            let label = root.label.as_deref().unwrap_or("").to_lowercase();
            let name = root.name.to_lowercase();
            if label.contains("arch") || name.contains("arch") {
                return "Arch Linux".into();
            }
            if label.contains("debian") {
                return "Debian".into();
            }
            if label.contains("ubuntu") {
                return "Ubuntu".into();
            }
            if label.contains("fedora") {
                return "Fedora".into();
            }
            if label.contains("nixos") {
                return "NixOS".into();
            }
            if label.contains("mint") {
                return "Linux Mint".into();
            }
        }
        "Unknown Linux".into()
    }

    // ── Scan — check if background disk detection finished ────────────────────

    pub fn check_scan(&mut self) {
        if self.scan_state != ScanState::Scanning {
            return;
        }
        if let Some(ref rx) = self.scan_rx
            && let Ok(disks) = rx.try_recv()
        {
            self.disks = disks;
            self.scan_state = ScanState::Done;
            self.scan_rx = None;
        }
    }

    // ── Exec log — drain pending lines from the repair thread ─────────────────

    pub fn drain_log(&mut self) {
        // Collect lines to avoid holding a borrow on self.log_rx while mutating self
        if self.log_rx.is_none() {
            return;
        }
        let mut new_lines: Vec<LogLine> = Vec::new();
        let mut done = false;
        if let Some(ref rx) = self.log_rx {
            while let Ok(line) = rx.try_recv() {
                if let LogKind::DiagnosisResult(summary, rec) = line.kind.clone() {
                    self.diagnosis_summary = summary;
                    self.recommended_action = rec;
                } else if line.kind == LogKind::Done {
                    done = true;
                } else {
                    new_lines.push(line);
                }
            }
        }
        for line in new_lines {
            if line.kind == LogKind::Step {
                self.exec_step += 1;
            }
            self.log_lines.push(line);
        }
        if done {
            self.exec_done = true;
            self.log_rx = None;
        }
    }

    // ── Start diagnosis — spawn background thread, switch to DiagnoseLog ──────

    pub fn start_diagnosis(&mut self) {
        let root = match &self.selected_root {
            Some(d) => d.clone(),
            None => return,
        };
        let efi = self.selected_efi.clone();
        let is_uefi = self.is_uefi;
        let disks = self.disks.clone();

        let (tx, rx) = std::sync::mpsc::channel::<LogLine>();
        self.log_rx = Some(rx);
        self.log_lines.clear();
        self.exec_step = 0;
        self.exec_done = false;
        self.exec_total = 4; // mount, detect, check grub, check fstab
        self.current_screen = CurrentScreen::DiagnoseLog;

        std::thread::spawn(move || {
            crate::repair::run_diagnosis(tx, root, efi, is_uefi, disks);
        });
    }

    // ── Start repair — spawn background thread, switch to ExecLog ─────────────

    pub fn start_repair(&mut self) {
        let action = match self.selected_action {
            Some(a) => a,
            None => return,
        };
        let root = match &self.selected_root {
            Some(d) => d.clone(),
            None => return,
        };
        let efi = self.selected_efi.clone();
        let is_uefi = self.is_uefi;

        let (tx, rx) = std::sync::mpsc::channel::<LogLine>();
        self.log_rx = Some(rx);
        self.log_lines.clear();
        self.exec_step = 0;
        self.exec_done = false;
        self.exec_total = match action {
            Action::FixGrubAndFstab => 9,
            Action::FixFstab => 5,
            _ => 7,
        };
        self.current_screen = CurrentScreen::ExecLog;

        std::thread::spawn(move || {
            crate::repair::run(tx, action, root, efi, is_uefi);
        });
    }
}
