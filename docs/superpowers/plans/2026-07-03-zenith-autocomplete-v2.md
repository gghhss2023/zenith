# Autocomplete v2 (Frequency + Recency Ranking) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade `History::suggest` from most-recent prefix match to a frequency + recency weighted score, per `docs/superpowers/specs/2026-07-03-zenith-autocomplete-v2-design.md`.

**Architecture:** Pure zenith-core change in `crates/zenith-core/src/history.rs`. `append()` stops dedup'ing (history file becomes an append log, format unchanged); an aggregate `HashMap<String, CommandStats>` is rebuilt on load/append; `suggest()` picks the highest-scored prefix match. FFI, Swift, ghost text untouched.

**Tech Stack:** Rust (std only). Tests live in the same file's `#[cfg(test)] mod tests`.

**Invariant (do not break):** the entry returned by `suggest(prefix)` must `starts_with` the raw prefix — FFI callers slice `entry[prefix.len()..]`.

---

### Task 1: Branch creation

**Files:** none

- [ ] **Step 1: Create the feature branch from main**

```bash
cd /Users/macosx/zenith
git checkout -b feature/autocomplete-v2
```

Expected: `Switched to a new branch 'feature/autocomplete-v2'`

- [ ] **Step 2: Commit the plan document**

```bash
git add docs/superpowers/plans/2026-07-03-zenith-autocomplete-v2.md
git commit -m "docs: autocomplete v2 implementation plan"
```

---

### Task 2: Append-log semantics (remove dedup)

**Files:**
- Modify: `crates/zenith-core/src/history.rs` (`append`, and the `append_dedups_and_ignores_empty` test)

- [ ] **Step 1: Replace the old dedup test and add a truncation test**

In `mod tests`, DELETE `append_dedups_and_ignores_empty` and add:

```rust
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
        for i in 0..10_000 {
            lines.push(format!("filler {}", i));
        }
        fs::write(&path, lines.join("\n") + "\n").unwrap();
        let mut h = History::load(path);
        h.append("new cmd");
        assert_eq!(h.entries.len(), MAX_ENTRIES);
        assert_eq!(h.suggest("ancient"), None);
    }
```

- [ ] **Step 2: Run tests to verify the new dup test fails**

Run: `cargo test -p zenith-core history -- --nocapture`
Expected: `append_keeps_duplicates_and_ignores_empty` FAILS (left: 1, right: 2) because `append` still dedups. `truncates_oldest_beyond_max` passes already (drain logic exists).

- [ ] **Step 3: Remove the dedup line from `append`**

In `append`, delete exactly one line:

```rust
        self.entries.retain(|e| e != command);
```

Resulting `append`:

```rust
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
        self.persist();
    }
```

- [ ] **Step 4: Run tests to verify all pass**

Run: `cargo test -p zenith-core`
Expected: all pass (existing `suggest_most_recent_first` still passes: with duplicates allowed, the reverse scan still finds the re-appended `git status` first).

- [ ] **Step 5: Commit**

```bash
git add crates/zenith-core/src/history.rs
git commit -m "feat: history file becomes append log (keep duplicates)"
```

---

### Task 3: Frecency scoring + aggregate map in suggest

**Files:**
- Modify: `crates/zenith-core/src/history.rs` (struct, `load`, `append`, `suggest`, new helpers, new tests)

- [ ] **Step 1: Write the failing scoring tests**

Add to `mod tests`:

```rust
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p zenith-core history -- --nocapture`
Expected: `frequency_beats_recency` FAILS (recency-only scan returns `git stash`). `old_occurrences_decay` FAILS (returns `make legacy`? No — reverse scan returns `make check`; it may pass by luck). `tie_breaks_by_recency` passes by luck. At least one failure is required before proceeding; `frequency_beats_recency` is the gate.

- [ ] **Step 3: Implement scoring**

Replace the top of the file (imports, consts, structs) with:

```rust
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
```

Replace `load`, `append`, `suggest` and add `build_stats`:

```rust
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
```

`default_path` and `persist` stay unchanged. Note: the tie-break on `last_index` inside `max_by` is mandatory — HashMap iteration order is random, so without it ties would be nondeterministic.

- [ ] **Step 4: Run the full zenith-core suite**

Run: `cargo test -p zenith-core`
Expected: all pass, including the pre-existing tests:
- `suggest_most_recent_first`: after the third append, `git status` has score 2.0 vs `git stash` 1.0 → `git status`. Before it, both 1.0 and `git stash` has larger `last_index` → `git stash`. Matches existing asserts.
- `suggest_skips_exact_match_and_empty_prefix`, `load_missing_file_gives_empty`, `persists_across_reload_with_0600`: semantics unchanged.

- [ ] **Step 5: Commit**

```bash
git add crates/zenith-core/src/history.rs
git commit -m "feat: rank suggestions by frequency + recency (frecency-lite)"
```

---

### Task 4: Workspace verification

**Files:** none (verification only)

- [ ] **Step 1: Full workspace tests**

Run: `cargo test --release 2>&1 | grep "test result"`
Expected: every line `ok`, 0 failed. (31+ tests total; zenith-core grows from 26 to 30.)

- [ ] **Step 2: Swift app still builds against the staticlib**

Run: `cargo build --release && cd Zenith && swift build && cd ..`
Expected: `Build complete!` — no FFI surface changed, this is a sanity check only.

- [ ] **Step 3: Commit (only if anything changed)**

```bash
git status --short
```

Expected: empty. Nothing to commit.

---

## Manual acceptance (after merge candidate is built)

1. `make install`, relaunch Zenith.
2. Run `git status` three times, `git stash list` once.
3. Type `git st` → ghost text should suggest `git status` (frequency wins), not `git stash list`.
4. → key accepts as before.
