use std::fs;
use std::path::PathBuf;

const MAX_ENTRIES: usize = 10_000;

pub struct History {
    entries: Vec<String>,
    path: PathBuf,
}

impl History {
    pub fn load(path: PathBuf) -> Self {
        let entries = fs::read_to_string(&path)
            .map(|s| {
                s.lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        Self { entries, path }
    }

    pub fn default_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/tmp"));
        PathBuf::from(home).join(".config/zenith/history")
    }

    pub fn append(&mut self, command: &str) {
        let command = command.trim();
        if command.is_empty() {
            return;
        }
        self.entries.retain(|e| e != command);
        self.entries.push(command.to_string());
        if self.entries.len() > MAX_ENTRIES {
            let excess = self.entries.len() - MAX_ENTRIES;
            self.entries.drain(..excess);
        }
        self.persist();
    }

    pub fn suggest(&self, prefix: &str) -> Option<&str> {
        if prefix.trim().is_empty() {
            return None;
        }
        self.entries
            .iter()
            .rev()
            .find(|e| e.starts_with(prefix) && e.as_str() != prefix)
            .map(|s| s.as_str())
    }

    fn persist(&self) {
        if let Some(dir) = self.path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let data = self.entries.join("\n") + "\n";
        let _ = fs::write(&self.path, data);
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("zenith_history_{}_{}", std::process::id(), name));
        let _ = fs::remove_file(&p);
        p
    }

    #[test]
    fn load_missing_file_gives_empty() {
        let h = History::load(temp_path("missing"));
        assert_eq!(h.suggest("ls"), None);
    }

    #[test]
    fn suggest_most_recent_first() {
        let mut h = History::load(temp_path("recent"));
        h.append("git status");
        h.append("git stash");
        assert_eq!(h.suggest("git st"), Some("git stash"));
        h.append("git status"); // moves to most-recent
        assert_eq!(h.suggest("git st"), Some("git status"));
    }

    #[test]
    fn suggest_skips_exact_match_and_empty_prefix() {
        let mut h = History::load(temp_path("exact"));
        h.append("ls");
        assert_eq!(h.suggest("ls"), None);
        assert_eq!(h.suggest(""), None);
        assert_eq!(h.suggest("   "), None);
        assert_eq!(h.suggest("l"), Some("ls"));
    }

    #[test]
    fn append_dedups_and_ignores_empty() {
        let mut h = History::load(temp_path("dedup"));
        h.append("make build");
        h.append("  ");
        h.append("make build");
        assert_eq!(h.entries.len(), 1);
    }

    #[test]
    fn persists_across_reload_with_0600() {
        use std::os::unix::fs::PermissionsExt;
        let path = temp_path("persist");
        let mut h = History::load(path.clone());
        h.append("cargo test");
        drop(h);
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        let h2 = History::load(path);
        assert_eq!(h2.suggest("cargo"), Some("cargo test"));
    }
}
