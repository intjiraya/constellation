use std::collections::HashMap;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use parking_lot::{Mutex, RwLock};
use tracing::error;

use crate::parser::SessionMeta;
use crate::scanner::{ProjectInfo, default_root, scan_projects};

#[derive(Default)]
pub struct Snapshot {
    pub projects: Vec<ProjectInfo>,
    pub by_project: HashMap<String, usize>,
    pub by_session_id: HashMap<String, SessionMeta>,
    pub last_scan: Option<DateTime<Utc>>,
}

impl Snapshot {
    pub fn project_count(&self) -> usize {
        self.projects.len()
    }
    pub fn session_count(&self) -> usize {
        self.by_session_id.len()
    }
    pub fn project_by_name(&self, name: &str) -> Option<&ProjectInfo> {
        self.by_project.get(name).map(|&i| &self.projects[i])
    }
}

pub struct SnapshotGuard<'a> {
    inner: parking_lot::RwLockReadGuard<'a, Snapshot>,
}

impl Deref for SnapshotGuard<'_> {
    type Target = Snapshot;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

struct ScanFlag<'a> {
    flag: &'a Mutex<bool>,
}

impl<'a> ScanFlag<'a> {
    fn set(flag: &'a Mutex<bool>) -> Self {
        *flag.lock() = true;
        Self { flag }
    }
}

impl Drop for ScanFlag<'_> {
    fn drop(&mut self) {
        *self.flag.lock() = false;
    }
}

#[derive(Clone)]
pub struct Index {
    root: PathBuf,
    state: Arc<RwLock<Snapshot>>,
    scanning: Arc<Mutex<bool>>,

    rebuild_lock: Arc<Mutex<()>>,
}

impl Index {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            state: Arc::new(RwLock::new(Snapshot::default())),
            scanning: Arc::new(Mutex::new(false)),
            rebuild_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn default_location() -> Self {
        Self::new(default_root())
    }

    pub fn root(&self) -> &PathBuf {
        &self.root
    }

    pub fn is_scanning(&self) -> bool {
        *self.scanning.lock()
    }

    pub fn rebuild(&self) {
        let _serialise = self.rebuild_lock.lock();
        let _flag = ScanFlag::set(&self.scanning);

        let projects = scan_projects(&self.root);
        let mut by_project = HashMap::with_capacity(projects.len());
        let mut by_session = HashMap::new();
        for (idx, p) in projects.iter().enumerate() {
            by_project.insert(p.sanitized_name.clone(), idx);
            for s in &p.sessions {
                by_session.insert(s.id.clone(), s.clone());
            }
        }
        let new_snap = Snapshot {
            projects,
            by_project,
            by_session_id: by_session,
            last_scan: Some(Utc::now()),
        };
        *self.state.write() = new_snap;
    }

    pub async fn rebuild_async(&self) -> Result<(), tokio::task::JoinError> {
        let me = self.clone();
        tokio::task::spawn_blocking(move || me.rebuild()).await
    }

    pub fn read(&self) -> SnapshotGuard<'_> {
        SnapshotGuard {
            inner: self.state.read(),
        }
    }
}

pub async fn warm_up(index: &Index) {
    if let Err(e) = index.rebuild_async().await {
        error!(error = %e, "initial scan panicked — serving empty index");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sessions")
    }

    fn seed(project_dir: &Path, fixture_name: &str, session_name: &str) {
        std::fs::create_dir_all(project_dir).unwrap();
        std::fs::copy(
            fixtures().join(fixture_name),
            project_dir.join(format!("{session_name}.jsonl")),
        )
        .unwrap();
    }

    #[test]
    fn rebuild_populates_snapshots() {
        let tmp = TempDir::new().unwrap();
        seed(&tmp.path().join("-a"), "minimal.jsonl", "minimal-uuid");
        seed(&tmp.path().join("-b"), "no_title.jsonl", "notitle-uuid");

        let idx = Index::new(tmp.path().to_owned());
        idx.rebuild();

        let snap = idx.read();
        assert_eq!(snap.project_count(), 2);
        assert_eq!(snap.session_count(), 2);
        assert!(snap.by_session_id.contains_key("minimal-uuid"));
        assert!(snap.by_session_id.contains_key("notitle-uuid"));
        assert!(snap.project_by_name("-a").is_some());
        assert!(snap.last_scan.is_some());
    }

    #[test]
    fn rebuild_is_idempotent_and_swaps_atomically() {
        let tmp = TempDir::new().unwrap();
        seed(&tmp.path().join("-a"), "minimal.jsonl", "minimal-uuid");
        let idx = Index::new(tmp.path().to_owned());
        idx.rebuild();
        let first_scan = idx.read().last_scan;
        std::thread::sleep(std::time::Duration::from_millis(2));
        idx.rebuild();
        let second_scan = idx.read().last_scan;
        assert!(second_scan > first_scan);
        assert_eq!(idx.read().session_count(), 1);
    }

    #[test]
    fn empty_index_safe_to_read() {
        let idx = Index::new("/this/does/not/exist".into());
        idx.rebuild();
        assert_eq!(idx.read().session_count(), 0);
        assert_eq!(idx.read().project_count(), 0);
    }

    #[test]
    fn scanning_flag_resets_after_normal_rebuild() {
        let tmp = TempDir::new().unwrap();
        let idx = Index::new(tmp.path().to_owned());
        idx.rebuild();
        assert!(!idx.is_scanning());
    }

    #[test]
    fn concurrent_rebuilds_are_serialised() {
        let tmp = TempDir::new().unwrap();
        seed(&tmp.path().join("-a"), "minimal.jsonl", "minimal-uuid");
        let idx = Index::new(tmp.path().to_owned());

        let i1 = idx.clone();
        let i2 = idx.clone();
        let t1 = std::thread::spawn(move || i1.rebuild());
        let t2 = std::thread::spawn(move || i2.rebuild());
        t1.join().unwrap();
        t2.join().unwrap();

        let snap = idx.read();
        assert_eq!(snap.project_count(), 1);
        assert_eq!(snap.session_count(), 1);
        assert!(!idx.is_scanning());
    }
}
