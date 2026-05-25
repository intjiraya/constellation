use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::parser::{parse_session_meta, SessionMeta, Usage};

#[derive(Debug, Clone, Serialize)]
pub struct ProjectInfo {
    pub sanitized_name: String,
    pub cwd: String,
    pub sessions: Vec<SessionMeta>,
}

impl ProjectInfo {
    pub fn total_messages(&self) -> usize {
        self.sessions.iter().map(|s| s.message_count).sum()
    }

    pub fn total_tools(&self) -> usize {
        self.sessions.iter().map(|s| s.tool_count).sum()
    }

    pub fn total_usage(&self) -> Usage {
        let mut u = Usage::default();
        for s in &self.sessions {
            u.add(&s.usage);
        }
        u
    }

    pub fn latest_at(&self) -> Option<DateTime<Utc>> {
        self.sessions.iter().filter_map(|s| s.last_at).max()
    }

pub fn display_path(&self) -> String {
        if let Some(home) = dirs::home_dir().and_then(|p| p.to_str().map(str::to_owned)) {
            if let Some(rest) = self.cwd.strip_prefix(&home) {
                return if rest.is_empty() {
                    "~".to_owned()
                } else {
                    format!("~{rest}")
                };
            }
        }
        self.cwd.clone()
    }
}

pub fn default_root() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join(".claude")
        .join("projects")
}

pub fn resolve_cwd_naive(sanitized: &str) -> String {
    if !sanitized.starts_with('-') {
        return sanitized.to_owned();
    }
    let trimmed = sanitized.trim_start_matches('-');
    let mut out = String::with_capacity(trimmed.len() + 1);
    out.push('/');
    out.push_str(&trimmed.replace('-', "/"));
    out
}

pub fn list_sessions_in_dir(project_dir: &Path) -> Vec<SessionMeta> {
    let mut metas: Vec<SessionMeta> = Vec::new();
    let entries = match std::fs::read_dir(project_dir) {
        Ok(e) => e,
        Err(_) => return metas,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        metas.push(parse_session_meta(&path));
    }
    metas.sort_by_key(|m| std::cmp::Reverse(m.last_at));
    metas
}

pub fn scan_projects(root: &Path) -> Vec<ProjectInfo> {
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut projects: Vec<ProjectInfo> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|entry| {
            let path = entry.path();
            let sanitized = entry.file_name().to_string_lossy().into_owned();
            let sessions = list_sessions_in_dir(&path);
            let cwd = sessions
                .iter()
                .find_map(|s| (!s.cwd.is_empty()).then(|| s.cwd.clone()))
                .unwrap_or_else(|| resolve_cwd_naive(&sanitized));
            ProjectInfo { sanitized_name: sanitized, cwd, sessions }
        })
        .collect();

    projects.sort_by_key(|p| std::cmp::Reverse(p.latest_at()));
    projects
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sessions")
    }

    fn seed(project_dir: &Path, fixture_name: &str, session_name: &str) {
        std::fs::create_dir_all(project_dir).unwrap();
        let src = fixtures().join(fixture_name);
        let dst = project_dir.join(format!("{session_name}.jsonl"));
        std::fs::copy(&src, &dst).unwrap();
    }

    #[test]
    fn empty_root_returns_empty() {
        let tmp = TempDir::new().unwrap();
        assert!(scan_projects(tmp.path()).is_empty());
    }

    #[test]
    fn finds_one_project() {
        let tmp = TempDir::new().unwrap();
        seed(&tmp.path().join("-home-test-proj"), "minimal.jsonl", "minimal-uuid");
        let out = scan_projects(tmp.path());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].sanitized_name, "-home-test-proj");
        assert_eq!(out[0].sessions.len(), 1);
        assert_eq!(out[0].sessions[0].id, "minimal-uuid");
    }

    #[test]
    fn resolves_cwd_from_session_record() {
        let tmp = TempDir::new().unwrap();
        seed(&tmp.path().join("-some-weird-thing"), "minimal.jsonl", "minimal-uuid");
        let out = scan_projects(tmp.path());
        
        assert_eq!(out[0].cwd, "/home/test/proj");
    }

    #[test]
    fn falls_back_to_naive_path_when_no_cwd_known() {
        let tmp = TempDir::new().unwrap();
        let proj = tmp.path().join("-home-test-foo");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::write(proj.join("no-content.jsonl"), "").unwrap();
        let out = scan_projects(tmp.path());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].cwd, "/home/test/foo");
    }

    #[test]
    fn sorts_projects_by_latest_session_activity() {
        let tmp = TempDir::new().unwrap();
        seed(&tmp.path().join("-old"), "minimal.jsonl", "a"); 
        seed(&tmp.path().join("-new"), "no_title.jsonl", "b"); 
        let out = scan_projects(tmp.path());
        let names: Vec<&str> = out.iter().map(|p| p.sanitized_name.as_str()).collect();
        assert_eq!(names, vec!["-new", "-old"]);
    }

    #[test]
    fn skips_non_jsonl_files_and_subdirs() {
        let tmp = TempDir::new().unwrap();
        let proj = tmp.path().join("-mixed");
        seed(&proj, "minimal.jsonl", "session");
        std::fs::write(proj.join("notes.txt"), "nope").unwrap();
        std::fs::create_dir_all(proj.join("memory")).unwrap();
        let out = scan_projects(tmp.path());
        assert_eq!(out[0].sessions.len(), 1);
    }

    #[test]
    fn handles_multiple_projects() {
        let tmp = TempDir::new().unwrap();
        for i in 0..5 {
            seed(
                &tmp.path().join(format!("-p{i}")),
                "minimal.jsonl",
                &format!("s{i}"),
            );
        }
        assert_eq!(scan_projects(tmp.path()).len(), 5);
    }

    #[test]
    fn list_sessions_in_dir_sorts_by_latest() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("-x");
        seed(&p, "minimal.jsonl", "early");   
        seed(&p, "no_title.jsonl", "later");  
        let sessions = list_sessions_in_dir(&p);
        assert_eq!(sessions[0].id, "notitle-uuid");
        assert_eq!(sessions[1].id, "minimal-uuid");
    }

    #[test]
    fn resolve_cwd_naive_known_cases() {
        assert_eq!(resolve_cwd_naive("-home-jiraya-code-personal"), "/home/jiraya/code/personal");
        assert_eq!(resolve_cwd_naive("-srv-app"), "/srv/app");
        assert_eq!(resolve_cwd_naive("-home-foo-bar-baz"), "/home/foo/bar/baz");
    }

    #[test]
    fn aggregated_stats() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("-stats");
        seed(&p, "minimal.jsonl", "a");
        seed(&p, "with_tools.jsonl", "b");
        let out = scan_projects(tmp.path());
        assert_eq!(out[0].total_messages(), 8); 
        assert_eq!(out[0].total_tools(), 1); 
    }

    #[test]
    fn missing_root_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("nope-not-there");
        assert!(scan_projects(&missing).is_empty());
    }

    #[test]
    fn display_path_replaces_home_with_tilde() {
        let home = dirs::home_dir().unwrap();
        let p = ProjectInfo {
            sanitized_name: "x".into(),
            cwd: home.join("code/personal").to_string_lossy().into_owned(),
            sessions: vec![],
        };
        assert_eq!(p.display_path(), "~/code/personal");

        let p2 = ProjectInfo {
            sanitized_name: "y".into(),
            cwd: home.to_string_lossy().into_owned(),
            sessions: vec![],
        };
        assert_eq!(p2.display_path(), "~");
    }
}
