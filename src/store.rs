//! Durable, single-writer access to `state.json`.
//!
//! Every write is a whole-file atomic replace: write a sibling temp file, fsync it, rename
//! it over the target, fsync the directory. With write-ahead ordering (invariant I5) a
//! crash then leaves state that is correct or conservatively stale, never ahead of reality.

use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::state::State;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path}: invalid state file: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("{path}: cannot serialize state: {source}")]
    Serialize {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

/// Reads and writes one project's `state.json`.
///
/// The Orchestrator is its only writer (invariant I2), so no locking is needed.
pub struct StateStore {
    path: PathBuf,
}

impl StateStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn exists(&self) -> bool {
        self.path.is_file()
    }

    pub fn load(&self) -> Result<State, Error> {
        let contents = fs::read_to_string(&self.path).map_err(|source| Error::Io {
            path: self.path.clone(),
            source,
        })?;

        serde_json::from_str(&contents).map_err(|source| Error::Parse {
            path: self.path.clone(),
            source,
        })
    }

    /// Replaces `state.json` atomically. A reader sees either the old file or the new one.
    pub fn save(&self, state: &State) -> Result<(), Error> {
        let mut json = serde_json::to_string_pretty(state).map_err(|source| Error::Serialize {
            path: self.path.clone(),
            source,
        })?;
        json.push('\n');

        let temp_path = self.temp_path();

        let mut temp = File::create(&temp_path).map_err(|source| Error::Io {
            path: temp_path.clone(),
            source,
        })?;
        temp.write_all(json.as_bytes())
            .and_then(|()| temp.sync_all())
            .map_err(|source| Error::Io {
                path: temp_path.clone(),
                source,
            })?;
        drop(temp);

        fs::rename(&temp_path, &self.path).map_err(|source| Error::Io {
            path: self.path.clone(),
            source,
        })?;

        // Without this the rename itself can be lost on power failure. Unix only: Windows
        // does not allow opening a directory as a file, and failing there would report an
        // error for a write that already succeeded.
        #[cfg(unix)]
        {
            let dir = self.parent_dir();
            File::open(&dir)
                .and_then(|handle| handle.sync_all())
                .map_err(|source| Error::Io { path: dir, source })?;
        }

        Ok(())
    }

    /// A sibling of the target, so two stores in one directory never share a temp file.
    fn temp_path(&self) -> PathBuf {
        let mut name = self
            .path
            .file_name()
            .unwrap_or_else(|| OsStr::new("state.json"))
            .to_os_string();
        name.push(".tmp");

        self.parent_dir().join(name)
    }

    fn parent_dir(&self) -> PathBuf {
        self.path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::state::{ExecutorState, NodeState, Phase};

    fn store_in(dir: &TempDir) -> StateStore {
        StateStore::new(dir.path().join("state.json"))
    }

    fn temp_of(store: &StateStore) -> PathBuf {
        store.temp_path()
    }

    #[test]
    fn saved_state_loads_back_unchanged() {
        let dir = TempDir::new().unwrap();
        let store = store_in(&dir);
        let mut state = State::new();
        state.phase = Phase::Executing;
        state
            .nodes
            .insert("n3".into(), NodeState::running(ExecutorState::Done));
        state.nodes.insert("n8".into(), NodeState::pending());

        store.save(&state).unwrap();

        assert_eq!(store.load().unwrap(), state);
    }

    #[test]
    fn save_replaces_an_existing_file_and_leaves_no_temp_behind() {
        let dir = TempDir::new().unwrap();
        let store = store_in(&dir);
        let mut state = State::new();
        store.save(&state).unwrap();

        state.phase = Phase::Designing;
        store.save(&state).unwrap();

        assert_eq!(store.load().unwrap().phase, Phase::Designing);
        assert!(!temp_of(&store).exists());
    }

    #[test]
    fn saved_file_is_pretty_printed_json_ending_in_a_newline() {
        let dir = TempDir::new().unwrap();
        let store = store_in(&dir);

        store.save(&State::new()).unwrap();

        let contents = fs::read_to_string(store.path()).unwrap();
        assert!(contents.ends_with("}\n"));
        assert!(contents.contains("\n  \"phase\": \"intake\""));
    }

    #[test]
    fn exists_reports_whether_the_run_has_state_yet() {
        let dir = TempDir::new().unwrap();
        let store = store_in(&dir);

        assert!(!store.exists());
        store.save(&State::new()).unwrap();
        assert!(store.exists());
    }

    #[test]
    fn loading_a_missing_file_names_the_path() {
        let dir = TempDir::new().unwrap();
        let store = store_in(&dir);

        let error = store.load().unwrap_err();

        assert!(matches!(error, Error::Io { .. }));
        assert!(error.to_string().contains("state.json"));
    }

    #[test]
    fn loading_malformed_json_reports_a_parse_error() {
        let dir = TempDir::new().unwrap();
        let store = store_in(&dir);
        fs::write(store.path(), "{ not json").unwrap();

        let error = store.load().unwrap_err();

        assert!(matches!(error, Error::Parse { .. }));
        assert!(error.to_string().contains("invalid state file"));
    }

    #[test]
    fn a_stale_temp_file_from_an_earlier_crash_is_overwritten() {
        let dir = TempDir::new().unwrap();
        let store = store_in(&dir);
        fs::write(temp_of(&store), "half-written garbage").unwrap();

        store.save(&State::new()).unwrap();

        assert_eq!(store.load().unwrap().phase, Phase::Intake);
        assert!(!temp_of(&store).exists());
    }

    #[test]
    fn a_save_never_touches_another_stores_temp_file() {
        let dir = TempDir::new().unwrap();
        let store = store_in(&dir);
        let neighbour = StateStore::new(dir.path().join("other.json"));
        fs::write(temp_of(&neighbour), "neighbour's in-flight write").unwrap();

        store.save(&State::new()).unwrap();

        assert_eq!(
            fs::read_to_string(temp_of(&neighbour)).unwrap(),
            "neighbour's in-flight write"
        );
    }

    #[test]
    fn an_unrecognized_phase_is_rejected_rather_than_guessed() {
        let dir = TempDir::new().unwrap();
        let store = store_in(&dir);
        fs::write(store.path(), r#"{"phase": "merging", "nodes": {}}"#).unwrap();

        let error = store.load().unwrap_err();

        assert!(matches!(error, Error::Parse { .. }));
    }

    #[test]
    fn an_unrecognized_node_status_is_rejected_rather_than_guessed() {
        let dir = TempDir::new().unwrap();
        let store = store_in(&dir);
        fs::write(
            store.path(),
            r#"{"phase": "executing", "nodes": {"n3": {"status": "awaiting_confirm", "executor": null}}}"#,
        )
        .unwrap();

        let error = store.load().unwrap_err();

        assert!(matches!(error, Error::Parse { .. }));
    }
}
