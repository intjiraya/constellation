use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::parser::Usage;
use crate::scanner::ProjectInfo;

#[derive(Debug, Clone, Serialize)]
pub struct IndexStats {
    pub projects: usize,
    pub sessions: usize,
    pub last_scan: Option<DateTime<Utc>>,
    pub scanning: bool,
    pub total_usage: Usage,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectOut {
    pub sanitized_name: String,
    pub cwd: String,
    pub display_path: String,
    pub session_count: usize,
    pub total_messages: usize,
    pub total_tools: usize,
    pub total_usage: Usage,
    pub latest_at: Option<DateTime<Utc>>,
}

impl From<&ProjectInfo> for ProjectOut {
    fn from(p: &ProjectInfo) -> Self {
        Self {
            sanitized_name: p.sanitized_name.clone(),
            cwd: p.cwd.clone(),
            display_path: p.display_path(),
            session_count: p.sessions.len(),
            total_messages: p.total_messages(),
            total_tools: p.total_tools(),
            total_usage: p.total_usage(),
            latest_at: p.latest_at(),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use serde_json::json;

    #[test]
    fn index_stats_json_shape_is_stable() {
        let s = IndexStats {
            projects: 3,
            sessions: 7,
            last_scan: None,
            scanning: false,
            total_usage: Usage {
                input: 1,
                cache_creation: 2,
                cache_read: 3,
                output: 4,
            },
        };
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(
            v,
            json!({
                "projects": 3,
                "sessions": 7,
                "last_scan": null,
                "scanning": false,
                "total_usage": {
                    "input": 1,
                    "cache_creation": 2,
                    "cache_read": 3,
                    "output": 4,
                },
            })
        );
    }

    #[test]
    fn project_out_json_shape_is_stable() {
        let p = ProjectOut {
            sanitized_name: "-x".into(),
            cwd: "/x".into(),
            display_path: "~/x".into(),
            session_count: 1,
            total_messages: 5,
            total_tools: 2,
            total_usage: Usage::default(),
            latest_at: None,
        };
        let v = serde_json::to_value(&p).unwrap();
        for field in [
            "sanitized_name",
            "cwd",
            "display_path",
            "session_count",
            "total_messages",
            "total_tools",
            "total_usage",
            "latest_at",
        ] {
            assert!(v.get(field).is_some(), "missing field {field}");
        }
    }
}
