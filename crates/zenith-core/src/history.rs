use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const MAX_ENTRIES: usize = 10_000;
const RECENT_WINDOW: usize = 1_000;
const MID_WINDOW: usize = 5_000;

struct CommandStats {
    score: f32,
    last_index: usize,
}

fn weight(age: usize) -> f32 {
    if age < RECENT_WINDOW {
        1.0
    } else if age < MID_WINDOW {
        0.5
    } else {
        0.25
    }
}

pub struct History {
    entries: Vec<String>,
    stats: HashMap<String, CommandStats>,
    path: PathBuf,
}

impl History {
    pub fn load(path: PathBuf) -> Self {
        let entries: Vec<String> = fs::read_to_string(&path)
            .map(|s| {
                s.lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        let stats = Self::build_stats(&entries);
        Self { entries, stats, path }
    }

    fn build_stats(entries: &[String]) -> HashMap<String, CommandStats> {
        let len = entries.len();
        let mut stats: HashMap<String, CommandStats> = HashMap::new();
        for (i, cmd) in entries.iter().enumerate() {
            let w = weight(len - 1 - i);
            match stats.get_mut(cmd) {
                Some(s) => {
                    s.score += w;
                    s.last_index = i;
                }
                None => {
                    stats.insert(cmd.clone(), CommandStats { score: w, last_index: i });
                }
            }
        }
        stats
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
        self.entries.push(command.to_string());
        if self.entries.len() > MAX_ENTRIES {
            let excess = self.entries.len() - MAX_ENTRIES;
            self.entries.drain(..excess);
        }
        self.stats = Self::build_stats(&self.entries);
        self.persist();
    }

    // The returned entry must start_with the raw prefix: callers slice the
    // remainder as entry[prefix.len()..], so never trim prefix before matching.
    pub fn suggest(&self, prefix: &str) -> Option<&str> {
        if prefix.trim().is_empty() {
            return None;
        }
        self.stats
            .iter()
            .filter(|(cmd, _)| cmd.starts_with(prefix) && cmd.as_str() != prefix)
            .max_by(|(_, a), (_, b)| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.last_index.cmp(&b.last_index))
            })
            .map(|(cmd, _)| cmd.as_str())
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
        h.append("git status"); // second occurrence, now most recent
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
    fn append_keeps_duplicates_and_ignores_empty() {
        let mut h = History::load(temp_path("log"));
        h.append("make build");
        h.append("  ");
        h.append("make build");
        assert_eq!(h.entries.len(), 2);
    }

    #[test]
    fn truncates_oldest_beyond_max() {
        let path = temp_path("truncate");
        let mut lines: Vec<String> = vec!["ancient cmd".to_string()];
        for i in 0..9_999 {
            lines.push(format!("filler {}", i));
        }
        fs::write(&path, lines.join("\n") + "\n").unwrap();
        let mut h = History::load(path);
        h.append("new cmd");
        assert_eq!(h.entries.len(), MAX_ENTRIES);
        assert_eq!(h.suggest("ancient"), None);
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

    #[test]
    fn frequency_beats_recency() {
        let mut h = History::load(temp_path("freq"));
        h.append("git status");
        h.append("git status");
        h.append("git status");
        h.append("git stash");
        assert_eq!(h.suggest("git st"), Some("git status"));
    }

    #[test]
    fn tie_breaks_by_recency() {
        let mut h = History::load(temp_path("tie"));
        h.append("foo bar");
        h.append("foo baz");
        assert_eq!(h.suggest("foo"), Some("foo baz"));
    }

    #[test]
    fn old_occurrences_decay() {
        let path = temp_path("decay");
        let mut lines: Vec<String> = Vec::new();
        for _ in 0..5 {
            lines.push("make legacy".to_string());
        }
        for i in 0..5_000 {
            lines.push(format!("filler {}", i));
        }
        lines.push("make check".to_string());
        lines.push("make check".to_string());
        fs::write(&path, lines.join("\n") + "\n").unwrap();
        let h = History::load(path);
        // legacy: 5 occurrences at age >= 5000 -> 5 * 0.25 = 1.25
        // check:  2 occurrences at age < 1000  -> 2 * 1.0  = 2.0
        assert_eq!(h.suggest("make"), Some("make check"));
    }
}
