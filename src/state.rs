use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppPhase {
    Idle,
    Connecting,
    Scanning,
    ScanComplete,
    Deleting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteMode {
    Trash,
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SenderInfo {
    pub email: String,
    pub count: usize,
}

#[derive(Debug, Clone)]
pub struct AppState {
    // Credentials (split for provider quick-select UX)
    pub email_user: String,
    pub email_domain: String,
    pub password: String,
    pub folder: String,

    // Scan settings
    pub scan_depth: u32,

    // State
    pub phase: AppPhase,
    pub delete_mode: DeleteMode,

    // Progress
    pub scan_progress: f32,
    pub scan_status: String,
    pub delete_progress: f32,
    pub delete_status: String,

    // Results
    pub total_emails: usize,
    pub senders: Vec<SenderInfo>,
    pub sender_selected: HashMap<String, bool>,

    // Errors
    pub error_message: Option<String>,

    // Folder discovery
    pub available_folders: Vec<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            email_user: String::new(),
            email_domain: String::new(),
            password: String::new(),
            folder: "INBOX".to_string(),
            scan_depth: 50_000,
            phase: AppPhase::Idle,
            delete_mode: DeleteMode::Trash,
            scan_progress: 0.0,
            scan_status: String::new(),
            delete_progress: 0.0,
            delete_status: String::new(),
            total_emails: 0,
            senders: Vec::new(),
            sender_selected: HashMap::new(),
            error_message: None,
            available_folders: Vec::new(),
        }
    }
}

impl AppState {
    pub fn email(&self) -> String {
        format!("{}{}", self.email_user, self.email_domain)
    }

    pub fn selected_senders(&self) -> Vec<&SenderInfo> {
        self.senders
            .iter()
            .filter(|s| self.sender_selected.get(&s.email).copied().unwrap_or(false))
            .collect()
    }

    pub fn selected_email_count(&self) -> usize {
        self.selected_senders().iter().map(|s| s.count).sum()
    }

    pub fn unique_senders(&self) -> usize {
        self.senders.len()
    }

    pub fn total_scanned(&self) -> usize {
        self.senders.iter().map(|s| s.count).sum()
    }
}
