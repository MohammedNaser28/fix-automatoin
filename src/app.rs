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
        // Take the receiver out to avoid holding a borrow on self while mutating
        let rx = match self.log_rx.take() {
            Some(rx) => rx,
            None => return,
        };

        let mut new_lines: Vec<LogLine> = Vec::new();
        let mut done = false;

        loop {
            match rx.try_recv() {
                Ok(line) => match line.kind {
                    LogKind::DiagnosisResult(summary, rec) => {
                        self.diagnosis_summary = summary;
                        self.recommended_action = rec;
                    }
                    LogKind::Done => {
                        done = true;
                        break;
                    }
                    _ => new_lines.push(line),
                },
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    // Thread still running - put the receiver back for next tick
                    self.log_rx = Some(rx);
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    // Sender dropped without a Done signal - the thread panicked.
                    if !done {
                        new_lines.push(LogLine::error("repair thread crashed before completion"));
                        done = true;
                    }
                    break;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn disk(name: &str, label: Option<&str>) -> DiskInfo {
        DiskInfo {
            name: name.into(),
            size: "10G".into(),
            fstype: Some("ext4".into()),
            label: label.map(|l| l.into()),
            uuid: Some("uuid-1".into()),
            mountpoint: None,
            is_efi: false,
            contents: None,
        }
    }

    fn test_app() -> App {
        let mut app = App::new();
        // Replace live scan results with deterministic fixtures
        app.disks = vec![disk("sda1", None), disk("sda2", None), disk("sda3", None)];
        app.scan_state = ScanState::Done;
        app.scan_rx = None;
        app
    }

    #[test]
    fn action_items_all_have_labels_and_descriptions() {
        for item in ACTION_ITEMS.iter().flatten() {
            assert!(!item.label().is_empty());
            assert!(!item.description().is_empty());
        }
    }

    #[test]
    fn action_cursor_starts_on_first_real_action() {
        let app = test_app();
        assert!(ACTION_ITEMS[app.action_cursor].is_some());
        assert_eq!(app.action_cursor, 1); // index 0 is the "repair" header
    }

    #[test]
    fn action_nav_skips_headers() {
        let mut app = test_app();
        // index 1 -> 2 -> 3 -> 4, next must jump over header at 5 to 6
        app.action_cursor = 4;
        app.action_next();
        assert_eq!(app.action_cursor, 6);

        // prev from 6 skips the header at 5 back to 4
        app.action_prev();
        assert_eq!(app.action_cursor, 4);
    }

    #[test]
    fn action_nav_wraps_forward_and_stops_at_top_backward() {
        let mut app = test_app();
        let last = ACTION_ITEMS.len() - 1; // last real action index
        assert!(ACTION_ITEMS[last].is_some());

        // Wrap from last back to first action
        app.action_cursor = last;
        app.action_next();
        assert_eq!(app.action_cursor, 1);

        // At first action, prev does not move onto the header
        app.action_prev();
        assert_eq!(app.action_cursor, 1);
    }

    #[test]
    fn confirm_focus_toggles_both_ways() {
        let mut app = test_app();
        assert_eq!(app.confirm_focus, ConfirmFocus::Confirm);
        app.toggle_confirm_buttons();
        assert_eq!(app.confirm_focus, ConfirmFocus::Back);
        app.toggle_confirm_buttons();
        assert_eq!(app.confirm_focus, ConfirmFocus::Confirm);
    }

    #[test]
    fn table_selection_wraps() {
        let mut app = test_app();

        app.table_state.select(Some(0));
        app.select_previous();
        assert_eq!(app.table_state.selected(), Some(2));

        app.select_next();
        assert_eq!(app.table_state.selected(), Some(0));

        app.select_next();
        assert_eq!(app.table_state.selected(), Some(1));
    }

    #[test]
    fn empty_disk_list_navigation_is_noop() {
        let mut app = test_app();
        app.disks.clear();
        app.table_state.select(Some(0));
        app.select_next();
        app.select_previous();
        assert_eq!(app.table_state.selected(), Some(0));
    }

    #[test]
    fn heuristic_distro_from_partition_label() {
        let cases = [
            ("arch-root", "Arch Linux"),
            ("debian", "Debian"),
            ("ubuntu", "Ubuntu"),
            ("fedora", "Fedora"),
            ("nixos", "NixOS"),
            ("mint", "Linux Mint"),
        ];
        for (label, expected) in cases {
            let mut app = test_app();
            app.disks[0].label = Some(label.into());
            app.selected_root = Some(app.disks[0].clone());
            assert_eq!(app.heuristic_distro(), expected, "label={label}");
        }
    }

    #[test]
    fn heuristic_distro_unknown_when_no_match() {
        let mut app = test_app();
        app.selected_root = Some(disk("sda2", Some("data")));
        assert_eq!(app.heuristic_distro(), "Unknown Linux");
    }

    #[test]
    fn detected_distro_takes_priority_over_heuristic() {
        let mut app = test_app();
        app.detected_distro = Some("Gentoo".into());
        app.selected_root = Some(disk("sda1", Some("arch")));
        assert_eq!(app.heuristic_distro(), "Gentoo");
    }

    #[test]
    fn drain_log_counts_steps_and_sets_done() {
        let mut app = test_app();
        let (tx, rx) = std::sync::mpsc::channel::<LogLine>();
        app.log_rx = Some(rx);
        app.exec_step = 0;
        app.exec_done = false;

        tx.send(LogLine::step("step one")).unwrap();
        tx.send(LogLine::ok("did something")).unwrap();
        tx.send(LogLine::step("step two")).unwrap();
        tx.send(LogLine::error("boom")).unwrap();
        tx.send(LogLine::done()).unwrap();
        drop(tx);

        app.drain_log();

        assert_eq!(app.exec_step, 2);
        assert_eq!(app.log_lines.len(), 4);
        assert!(app.exec_done);
        assert!(app.log_rx.is_none());
    }

    #[test]
    fn drain_log_extracts_diagnosis_result() {
        let mut app = test_app();
        let (tx, rx) = std::sync::mpsc::channel::<LogLine>();
        app.log_rx = Some(rx);

        tx.send(LogLine {
            kind: LogKind::DiagnosisResult(vec!["Diagnosis: broken".into()], Some(Action::FixGrub)),
            text: String::new(),
        })
        .unwrap();
        tx.send(LogLine::done()).unwrap();
        drop(tx);

        app.drain_log();

        assert_eq!(app.diagnosis_summary, vec!["Diagnosis: broken".to_string()]);
        assert_eq!(app.recommended_action, Some(Action::FixGrub));
        // DiagnosisResult is not rendered as a log line
        assert!(app.log_lines.is_empty());
        assert!(app.exec_done);
    }

    #[test]
    fn drain_log_without_receiver_is_noop() {
        let mut app = test_app();
        app.drain_log();
        assert!(!app.exec_done);
    }

    #[test]
    fn drain_log_recovers_from_crashed_thread() {
        let mut app = test_app();
        let (tx, rx) = std::sync::mpsc::channel::<LogLine>();
        app.log_rx = Some(rx);
        app.exec_done = false;

        tx.send(LogLine::step("mounting")).unwrap();
        // Simulate a panic: sender dropped without ever sending Done
        drop(tx);

        app.drain_log();

        assert!(app.exec_done, "UI must not hang when the thread panics");
        assert_eq!(app.exec_step, 1);
        let last = app.log_lines.last().unwrap();
        assert_eq!(last.kind, LogKind::Error);
        assert!(last.text.contains("crashed"));
        // Receiver must be gone so drain is a no-op afterwards
        assert!(app.log_rx.is_none());
    }

    #[test]
    fn check_scan_consumes_channel_once() {
        let mut app = test_app();
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(vec![disk("vda1", None)]).unwrap();
        app.scan_rx = Some(rx);
        app.scan_state = ScanState::Scanning;

        app.check_scan();
        assert_eq!(app.scan_state, ScanState::Done);
        assert_eq!(app.disks.len(), 1);
        assert_eq!(app.disks[0].name, "vda1");

        // Second call is a no-op
        app.check_scan();
        assert_eq!(app.disks.len(), 1);
    }
}
