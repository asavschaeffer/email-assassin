#[derive(Debug, Clone)]
pub struct ImapProvider {
    pub host: &'static str,
    pub port: u16,
    pub trash_folder: &'static str,
}

impl ImapProvider {
    pub fn from_email(email: &str) -> Self {
        let domain = email
            .rsplit('@')
            .next()
            .unwrap_or("")
            .to_lowercase();

        if domain.contains("outlook") || domain.contains("hotmail") || domain.contains("live.com")
        {
            Self {
                host: "imap-mail.outlook.com",
                port: 993,
                trash_folder: "Deleted",
            }
        } else if domain.contains("yahoo") {
            Self {
                host: "imap.mail.yahoo.com",
                port: 993,
                trash_folder: "Trash",
            }
        } else if domain.contains("icloud") || domain.contains("me.com") || domain.contains("mac.com") {
            Self {
                host: "imap.mail.me.com",
                port: 993,
                trash_folder: "Deleted Messages",
            }
        } else {
            // Default: Gmail
            Self {
                host: "imap.gmail.com",
                port: 993,
                trash_folder: "[Gmail]/Trash",
            }
        }
    }
}
