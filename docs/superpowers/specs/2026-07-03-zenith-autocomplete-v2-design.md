# Zenith Autocomplete v2: Frequency + Recency Ranking

**Date:** 2026-07-03
**Status:** Approved
**Scope decision:** Option A — keep prefix matching and the existing ghost-text UX; upgrade ranking from "most recent only" to a frequency + recency weighted score. Fuzzy matching (Option B) and a candidate dropdown (Option C) are explicitly out of scope.

## Background

`History::suggest(prefix)` (crates/zenith-core/src/history.rs) currently returns the most recently used entry that starts with the prefix. `append()` dedups: a repeated command is removed from its old position and re-appended, so the history file stores each command at most once and all frequency information is destroyed.

The ghost-text contract (unchanged): the returned entry must `starts_with` the raw prefix, because callers (`zn_terminal_render`, `zn_terminal_accept_suggestion` in crates/zenith-ffi) slice the remainder as `entry[prefix.len()..]`.

## Design

### Storage: append log instead of dedup'd list

- `append()` no longer dedups. Every executed command appends one line.
- File format unchanged: one command per line, most recent last, `~/.config/zenith/history`, mode 0600.
- Old files load as-is (a dedup'd file is just a log where every command has count 1). No migration.
- `MAX_ENTRIES = 10_000` now caps log lines (not unique commands). Overflow drains the oldest lines, so stale commands decay naturally.

### Scoring: frecency-lite by log position (no timestamps)

Age of an occurrence = distance from the end of the log (0 = newest line).

| Age (lines from end) | Weight per occurrence |
|---|---|
| 0–999 | 1.0 |
| 1000–4999 | 0.5 |
| ≥5000 | 0.25 |

- `score(command) = Σ weight(occurrence)` over all its occurrences in the log.
- Candidate set = entries where `entry.starts_with(prefix) && entry != prefix`.
- Winner = highest score; tie-break = most recent last use (larger last index wins).

### In-memory aggregation

- `History` keeps the raw `entries: Vec<String>` log plus an aggregate `HashMap<String, CommandStats>` where `CommandStats { score: f32, last_index: usize }`.
- The map is rebuilt in full on `load()` and on every `append()` (O(10k) at human command rate — negligible; appending shifts the age of every prior occurrence, so incremental updates buy nothing but complexity).
- `suggest(prefix)` iterates the aggregate map, not the raw log, so per-render-frame cost is bounded by unique-command count.
- `suggest` signature and the starts_with contract are unchanged.

### What does not change

- FFI surface, Swift code, ghost-text rendering, →-accept path: untouched. This is a pure zenith-core change.
- Empty/whitespace prefix returns None; exact match (entry == prefix) is never suggested.
- Persistence: `persist()` writes the full log joined by newlines, 0600.

## Testing

New tests:
1. Frequency beats recency: `git status` ×3 then `git stash` ×1 → `suggest("git st")` = `git status`.
2. Recency tie-break: `foo bar` ×1, `foo baz` ×1 → `suggest("foo")` = the later one.
3. Recency-weight decay: command with many old occurrences (age ≥5000, weight 0.25 each) loses to a command with fewer recent occurrences (weight 1.0 each). Construct via bulk-append filler lines.
4. Log semantics: appending the same command twice yields 2 entries (replaces the old `append_dedups_and_ignores_empty` expectation; empty/whitespace still ignored).
5. Truncation: exceeding MAX_ENTRIES drains oldest lines.

Existing tests preserved (with the one adjustment above): old-file compatibility, empty-prefix None, exact-match skip, 0600 persistence round-trip.

## Non-goals

- Fuzzy/subsequence matching and replacement-style accept.
- Dropdown/candidate-list UI.
- Importing shell HISTFILE.
- Timestamps in the history file.
