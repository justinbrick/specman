use crate::error::{Result, SpecmanMcpError};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use specman::workspace::WorkspacePaths;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

/// Ensures every filesystem access remains scoped to a single workspace root.
#[derive(Clone, Debug)]
pub struct WorkspaceSessionGuard {
    paths: WorkspacePaths,
}

impl WorkspaceSessionGuard {
    /// Builds a guard for the provided workspace paths.
    pub fn new(paths: WorkspacePaths) -> Self {
        Self { paths }
    }

    /// Returns the underlying workspace metadata.
    pub fn workspace(&self) -> &WorkspacePaths {
        &self.paths
    }

    /// Canonicalizes the supplied path and ensures it is located under the workspace root.
    pub fn normalize(&self, path: impl AsRef<Path>) -> Result<PathBuf> {
        let candidate = path.as_ref();
        let absolute = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            self.paths.root().join(candidate)
        };

        let canonical = std::fs::canonicalize(&absolute)?;
        if !canonical.starts_with(self.paths.root()) {
            return Err(SpecmanMcpError::workspace(format!(
                "path {} escapes workspace {}",
                canonical.display(),
                self.paths.root().display()
            )));
        }

        Ok(canonical)
    }
}

/// Lightweight session metadata tracked for MCP lifecycle/telemetry purposes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: String,
    pub workspace_root: PathBuf,
    pub protocol_version: String,
    pub started_at: OffsetDateTime,
    pub last_heartbeat: OffsetDateTime,
}

impl SessionRecord {
    fn new(session_id: impl Into<String>, guard: &WorkspaceSessionGuard, protocol: impl Into<String>) -> Self {
        let now = OffsetDateTime::now_utc();
        Self {
            session_id: session_id.into(),
            workspace_root: guard.workspace().root().to_path_buf(),
            protocol_version: protocol.into(),
            started_at: now,
            last_heartbeat: now,
        }
    }

    fn heartbeat(&mut self) {
        self.last_heartbeat = OffsetDateTime::now_utc();
    }
}

/// Tracks active MCP sessions to enforce the single-workspace constraint.
#[derive(Default)]
pub struct SessionManager {
    sessions: DashMap<String, SessionRecord>,
}

impl SessionManager {
    /// Registers a new session. Returns an error if the session already exists.
    pub fn start_session(
        &self,
        session_id: impl Into<String>,
        guard: &WorkspaceSessionGuard,
        protocol_version: impl Into<String>,
    ) -> Result<SessionRecord> {
        let session_id = session_id.into();
        if self.sessions.contains_key(&session_id) {
            return Err(SpecmanMcpError::workspace(format!(
                "session {session_id} already exists"
            )));
        }

        let record = SessionRecord::new(&session_id, guard, protocol_version);
        self.sessions.insert(session_id.clone(), record.clone());
        Ok(record)
    }

    /// Marks the session as finished and removes it from the manager.
    pub fn finish_session(&self, session_id: &str) -> Option<SessionRecord> {
        self.sessions.remove(session_id).map(|(_, record)| record)
    }

    /// Updates the heartbeat timestamp for the given session.
    pub fn heartbeat(&self, session_id: &str) -> Result<()> {
        let mut entry = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SpecmanMcpError::workspace(format!("session {session_id} not found")))?;
        entry.heartbeat();
        Ok(())
    }

    /// Returns the current snapshot of a session, if present.
    pub fn get(&self, session_id: &str) -> Option<SessionRecord> {
        self.sessions.get(session_id).map(|entry| entry.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specman::workspace::WorkspacePaths;
    use tempfile::tempdir;

    fn guard() -> WorkspaceSessionGuard {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        std::fs::create_dir_all(root.join(".specman")).unwrap();
        WorkspaceSessionGuard::new(WorkspacePaths::new(root.clone(), root.join(".specman")))
    }

    #[test]
    fn session_manager_tracks_lifecycle() {
        let guard = guard();
        let manager = SessionManager::default();
        manager
            .start_session("abc", &guard, "2025-11-25")
            .expect("start");
        manager.heartbeat("abc").expect("heartbeat");
        assert!(manager.get("abc").is_some());
        let record = manager.finish_session("abc").expect("finish");
        assert_eq!(record.session_id, "abc");
        assert!(manager.get("abc").is_none());
    }
}
