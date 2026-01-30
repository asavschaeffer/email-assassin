use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppPhase {
    Idle,
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
    // Credentials
    pub email: String,
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
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            email: String::new(),
            password: String::new(),
            folder: "INBOX".to_string(),
            scan_depth: 0,
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
        }
    }
}

impl AppState {
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
