use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::warn;

pub const PLACEHOLDER_TITLE: &str = "(empty session)";
pub const UNTITLED: &str = "(untitled)";

const NOISE_PREFIXES: &[&str] = &[
    "<command-name>",
    "<command-message>",
    "<command-args>",
    "<local-command-caveat>",
    "<local-command-stdout>",
    "<system-reminder>",
];

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Block {
    Text {
        text: String,
    },
    Thinking {
        text: String,
    },
    ToolUse {
        tool_name: String,
        tool_input: Value,
        tool_use_id: String,
    },
    ToolResult {
        tool_use_id: String,
        tool_output: String,
        is_error: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize)]
pub struct Turn {
    pub uuid: String,
    pub role: Role,
    pub timestamp: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model: String,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub struct Usage {
    pub input: u64,
    pub cache_creation: u64,
    pub cache_read: u64,
    pub output: u64,
}

impl Usage {
    pub fn total_input(&self) -> u64 {
        self.input + self.cache_creation + self.cache_read
    }

    pub fn total(&self) -> u64 {
        self.total_input() + self.output
    }

    pub fn cache_hit_ratio(&self) -> Option<f32> {
        let total_in = self.total_input();
        if total_in == 0 {
            None
        } else {
            Some(self.cache_read as f32 / total_in as f32)
        }
    }
    pub fn add(&mut self, other: &Usage) {
        self.input += other.input;
        self.cache_creation += other.cache_creation;
        self.cache_read += other.cache_read;
        self.output += other.output;
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionMeta {
    pub id: String,
    pub cwd: String,
    pub project_dir: String,
    pub title: String,
    pub model: String,
    pub started_at: Option<DateTime<Utc>>,
    pub last_at: Option<DateTime<Utc>>,
    pub message_count: usize,
    pub tool_count: usize,
    pub snippet: String,
    pub path: PathBuf,
    pub size: u64,
    pub usage: Usage,

    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub skipped_lines: usize,
}

fn is_zero_usize(n: &usize) -> bool {
    *n == 0
}

#[derive(Debug, Clone, Serialize)]
pub struct Session {
    #[serde(flatten)]
    pub meta: SessionMeta,
    pub turns: Vec<Turn>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAiTitle {
    ai_title: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTurn {
    #[serde(default)]
    message: Option<RawMessage>,
    #[serde(default)]
    uuid: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    is_meta: bool,
    #[serde(default, rename = "cwd")]
    _cwd: Option<String>,
    #[serde(default, rename = "sessionId")]
    _session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawMessage {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    content: Value,
    #[serde(default)]
    usage: Option<RawUsage>,
}

#[derive(Debug, Deserialize, Default)]
struct RawUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
}

impl From<&RawUsage> for Usage {
    fn from(r: &RawUsage) -> Self {
        Usage {
            input: r.input_tokens,
            output: r.output_tokens,
            cache_creation: r.cache_creation_input_tokens,
            cache_read: r.cache_read_input_tokens,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum RawBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        #[serde(default)]
        input: Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        #[serde(default)]
        content: Value,
        #[serde(default)]
        is_error: bool,
    },
}

fn parse_timestamp(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

fn real_user_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(arr) => arr
            .iter()
            .filter_map(|item| match item.get("type")?.as_str()? {
                "text" => item.get("text")?.as_str().map(str::to_owned),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

fn is_noise(text: &str) -> bool {
    let trimmed = text.trim_start();
    NOISE_PREFIXES.iter().any(|p| trimmed.starts_with(p))
}

fn clean_title(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(120)
        .collect()
}

fn extract_blocks(content: Value) -> Vec<Block> {
    let mut out = Vec::new();
    match content {
        Value::String(s) if !s.is_empty() => out.push(Block::Text { text: s }),
        Value::Array(arr) => {
            for item in arr {
                if let Ok(b) = serde_json::from_value::<RawBlock>(item) {
                    out.push(convert_block(b));
                }
            }
        }
        _ => {}
    }
    out
}

fn count_tool_uses(content: &Value) -> usize {
    match content {
        Value::Array(arr) => arr
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some("tool_use"))
            .count(),
        _ => 0,
    }
}

fn convert_block(raw: RawBlock) -> Block {
    match raw {
        RawBlock::Text { text } => Block::Text { text },
        RawBlock::Thinking { thinking } => Block::Thinking { text: thinking },
        RawBlock::ToolUse { id, name, input } => Block::ToolUse {
            tool_name: name,
            tool_input: input,
            tool_use_id: id,
        },
        RawBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => Block::ToolResult {
            tool_use_id,
            tool_output: stringify_tool_content(content),
            is_error,
        },
    }
}

fn stringify_tool_content(v: Value) -> String {
    match v {
        Value::String(s) => s,
        Value::Array(arr) => arr
            .iter()
            .filter_map(|x| x.get("text").and_then(Value::as_str).map(str::to_owned))
            .collect::<Vec<_>>()
            .join(" "),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn iter_events(path: &Path) -> Box<dyn Iterator<Item = (Option<Value>, String)>> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            warn!(path = %path.display(), error = %e, "failed to open session file");
            return Box::new(std::iter::empty());
        }
    };
    let reader = BufReader::new(file);
    Box::new(
        reader
            .lines()
            .map_while(Result::ok)
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let parsed = serde_json::from_str::<Value>(&l).ok();
                (parsed, l)
            }),
    )
}

struct Aggregated {
    ai_title: String,
    cwd: String,
    model: String,
    session_id: String,
    started_at: Option<DateTime<Utc>>,
    last_at: Option<DateTime<Utc>>,
    message_count: usize,
    tool_count: usize,
    first_user_text: String,
    turns: Vec<Turn>,
    usage: Usage,
    skipped_lines: usize,
    file_size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    MetaOnly,
    Full,
}

fn aggregate(path: &Path, mode: Mode) -> Aggregated {
    let mut agg = Aggregated {
        ai_title: String::new(),
        cwd: String::new(),
        model: String::new(),
        session_id: String::new(),
        started_at: None,
        last_at: None,
        message_count: 0,
        tool_count: 0,
        first_user_text: String::new(),
        turns: Vec::new(),
        usage: Usage::default(),
        skipped_lines: 0,
        file_size: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
    };

    for (event_opt, _line) in iter_events(path) {
        let Some(event) = event_opt else {
            agg.skipped_lines += 1;
            continue;
        };

        if agg.session_id.is_empty() {
            if let Some(s) = event.get("sessionId").and_then(Value::as_str) {
                agg.session_id = s.to_owned();
            }
        }

        let kind = event
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();

        if kind == "ai-title" {
            if let Ok(t) = serde_json::from_value::<RawAiTitle>(event) {
                if let Some(title) = t.ai_title {
                    if !title.is_empty() {
                        agg.ai_title = title;
                    }
                }
            }
            continue;
        }

        if agg.cwd.is_empty() {
            if let Some(c) = event.get("cwd").and_then(Value::as_str) {
                agg.cwd = c.to_owned();
            }
        }

        if kind != "user" && kind != "assistant" {
            continue;
        }

        let raw: RawTurn = match serde_json::from_value(event) {
            Ok(r) => r,
            Err(_) => continue,
        };

        if kind == "user" && raw.is_meta {
            continue;
        }

        let role = if kind == "user" {
            Role::User
        } else {
            Role::Assistant
        };
        let ts = raw.timestamp.as_deref().and_then(parse_timestamp);

        if let Some(t) = ts {
            agg.started_at = Some(match agg.started_at {
                Some(prev) => std::cmp::min(prev, t),
                None => t,
            });
            agg.last_at = Some(match agg.last_at {
                Some(prev) => std::cmp::max(prev, t),
                None => t,
            });
        }

        let msg = raw.message.unwrap_or(RawMessage {
            model: None,
            content: Value::Null,
            usage: None,
        });

        if role == Role::Assistant {
            if let Some(m) = &msg.model {
                if !m.is_empty() {
                    agg.model = m.clone();
                }
            }
            if let Some(u) = &msg.usage {
                agg.usage.add(&Usage::from(u));
            }
        }

        if role == Role::User && agg.first_user_text.is_empty() {
            let candidate = real_user_text(&msg.content).trim().to_owned();
            if !candidate.is_empty() && !is_noise(&candidate) {
                agg.first_user_text = candidate;
            }
        }

        match mode {
            Mode::Full => {
                let blocks = extract_blocks(msg.content);
                agg.tool_count += blocks
                    .iter()
                    .filter(|b| matches!(b, Block::ToolUse { .. }))
                    .count();
                let turn_model = if role == Role::Assistant {
                    msg.model.unwrap_or_default()
                } else {
                    String::new()
                };
                agg.turns.push(Turn {
                    uuid: raw.uuid.unwrap_or_default(),
                    role,
                    timestamp: ts.unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).unwrap()),
                    model: turn_model,
                    blocks,
                });
            }
            Mode::MetaOnly => {
                agg.tool_count += count_tool_uses(&msg.content);
            }
        }

        agg.message_count += 1;
    }

    if agg.skipped_lines > 0 {
        warn!(
            path = %path.display(),
            skipped = agg.skipped_lines,
            "skipped malformed JSONL lines"
        );
    }

    agg
}

fn build_meta(path: &Path, agg: &Aggregated) -> SessionMeta {
    let title = if !agg.ai_title.is_empty() {
        clean_title(&agg.ai_title)
    } else if !agg.first_user_text.is_empty() {
        clean_title(&agg.first_user_text)
    } else if !agg.turns.is_empty() || agg.message_count > 0 {
        UNTITLED.to_owned()
    } else {
        PLACEHOLDER_TITLE.to_owned()
    };

    let snippet = if agg.first_user_text.is_empty() {
        String::new()
    } else {
        let mut s = clean_title(&agg.first_user_text);
        s.truncate(240);
        s
    };

    let id = if agg.session_id.is_empty() {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_owned()
    } else {
        agg.session_id.clone()
    };

    let project_dir = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_owned();

    SessionMeta {
        id,
        cwd: agg.cwd.clone(),
        project_dir,
        title,
        model: agg.model.clone(),
        started_at: agg.started_at,
        last_at: agg.last_at,
        message_count: agg.message_count,
        tool_count: agg.tool_count,
        snippet,
        path: path.to_owned(),
        size: agg.file_size,
        usage: agg.usage,
        skipped_lines: agg.skipped_lines,
    }
}

pub fn parse_session_meta(path: &Path) -> SessionMeta {
    let agg = aggregate(path, Mode::MetaOnly);
    build_meta(path, &agg)
}

pub fn parse_session(path: &Path) -> Session {
    let agg = aggregate(path, Mode::Full);
    let meta = build_meta(path, &agg);
    Session {
        meta,
        turns: agg.turns,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sessions")
    }

    fn fp(name: &str) -> PathBuf {
        fixtures().join(name)
    }

    #[test]
    fn meta_minimal() {
        let m = parse_session_meta(&fp("minimal.jsonl"));
        assert_eq!(m.id, "minimal-uuid");
        assert_eq!(m.cwd, "/home/test/proj");
        assert_eq!(m.title, "Test conversation about parsers");
        assert_eq!(m.model, "claude-opus-4-7");
        assert_eq!(m.message_count, 4);
        assert_eq!(m.tool_count, 0);
        assert_eq!(m.skipped_lines, 0);
        assert!(m.snippet.contains("JSONL") || m.snippet.contains("Hello"));
    }

    #[test]
    fn meta_with_tools() {
        let m = parse_session_meta(&fp("with_tools.jsonl"));
        assert_eq!(m.id, "tools-uuid");
        assert_eq!(m.cwd, "/srv/app");
        assert_eq!(m.title, "Look at the project structure");
        assert_eq!(m.model, "claude-sonnet-4-6");
        assert_eq!(m.tool_count, 1);
    }

    #[test]
    fn meta_no_title_falls_back_to_first_user() {
        let m = parse_session_meta(&fp("no_title.jsonl"));
        assert!(m.title.starts_with("This session has no ai-title"));
        assert_eq!(m.model, "claude-haiku-4-5-20251001");
    }

    #[test]
    fn meta_skips_meta_user_messages() {
        let m = parse_session_meta(&fp("with_meta.jsonl"));
        assert_eq!(m.title, "Session with meta noise");
        assert_eq!(m.message_count, 2);
        assert!(m.snippet.contains("Real first"));
    }

    #[test]
    fn meta_empty_file_returns_placeholder() {
        let m = parse_session_meta(&fp("empty.jsonl"));
        assert_eq!(m.id, "empty");
        assert_eq!(m.message_count, 0);
        assert_eq!(m.title, "(empty session)");
    }

    #[test]
    fn meta_malformed_lines_are_skipped_and_counted() {
        let m = parse_session_meta(&fp("malformed.jsonl"));
        assert_eq!(m.title, "Survives broken lines");
        assert_eq!(m.message_count, 2);
        assert_eq!(m.cwd, "/x");

        assert_eq!(m.skipped_lines, 2);
    }

    #[test]
    fn meta_has_timestamps() {
        let m = parse_session_meta(&fp("minimal.jsonl"));
        let start = m.started_at.expect("started_at");
        let last = m.last_at.expect("last_at");
        assert!(last >= start);
    }

    #[test]
    fn meta_returns_path_and_size() {
        let m = parse_session_meta(&fp("minimal.jsonl"));
        assert_eq!(m.path, fp("minimal.jsonl"));
        assert!(m.size > 0);
    }

    #[test]
    fn full_parse_minimal() {
        let s = parse_session(&fp("minimal.jsonl"));
        assert_eq!(s.meta.id, "minimal-uuid");
        assert_eq!(s.turns.len(), 4);
        assert_eq!(s.turns[0].role, Role::User);
        assert_eq!(s.turns[1].role, Role::Assistant);
        assert_eq!(s.turns[1].model, "claude-opus-4-7");

        match &s.turns[0].blocks[0] {
            Block::Text { text } => assert_eq!(text, "Hello, can you explain JSONL?"),
            other => panic!("expected Text, got {other:?}"),
        }
        match &s.turns[1].blocks[0] {
            Block::Text { text } => assert!(text.starts_with("JSONL is JSON Lines")),
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn full_parse_extracts_tool_use() {
        let s = parse_session(&fp("with_tools.jsonl"));
        let tool_use = s
            .turns
            .iter()
            .flat_map(|t| &t.blocks)
            .find_map(|b| match b {
                Block::ToolUse {
                    tool_name,
                    tool_input,
                    ..
                } => Some((tool_name.clone(), tool_input.clone())),
                _ => None,
            })
            .expect("tool_use block");
        assert_eq!(tool_use.0, "Bash");
        assert_eq!(tool_use.1["command"], "ls -la");

        let tool_result = s
            .turns
            .iter()
            .flat_map(|t| &t.blocks)
            .find_map(|b| match b {
                Block::ToolResult {
                    tool_use_id,
                    tool_output,
                    is_error,
                } => Some((tool_use_id.clone(), tool_output.clone(), *is_error)),
                _ => None,
            })
            .expect("tool_result block");
        assert_eq!(tool_result.0, "tu-1");
        assert!(tool_result.1.contains("README.md"));
        assert!(!tool_result.2);
    }

    #[test]
    fn full_parse_extracts_thinking() {
        let s = parse_session(&fp("with_tools.jsonl"));
        let thinking = s
            .turns
            .iter()
            .flat_map(|t| &t.blocks)
            .filter_map(|b| match b {
                Block::Thinking { text } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(thinking.len(), 1);
        assert!(thinking[0].contains("use ls"));
    }

    #[test]
    fn full_parse_skips_meta_records() {
        let s = parse_session(&fp("with_meta.jsonl"));
        assert_eq!(s.turns.len(), 2);
        assert_eq!(s.turns[0].role, Role::User);
        match &s.turns[0].blocks[0] {
            Block::Text { text } => assert_eq!(text, "Real first user message."),
            other => panic!("unexpected block: {other:?}"),
        }
    }

    #[test]
    fn full_parse_malformed_still_works() {
        let s = parse_session(&fp("malformed.jsonl"));
        assert_eq!(s.turns.len(), 2);
        assert_eq!(s.meta.title, "Survives broken lines");
        assert_eq!(s.meta.skipped_lines, 2);
    }

    #[test]
    fn full_parse_empty() {
        let s = parse_session(&fp("empty.jsonl"));
        assert!(s.turns.is_empty());
        assert_eq!(s.meta.title, "(empty session)");
    }

    #[test]
    fn usage_is_aggregated_across_assistant_turns() {
        let m = parse_session_meta(&fp("with_usage.jsonl"));
        assert_eq!(m.usage.input, 15);
        assert_eq!(m.usage.cache_creation, 500);
        assert_eq!(m.usage.cache_read, 2500);
        assert_eq!(m.usage.output, 300);
    }

    #[test]
    fn usage_helpers_compute_totals_and_ratio() {
        let m = parse_session_meta(&fp("with_usage.jsonl"));
        assert_eq!(m.usage.total_input(), 15 + 500 + 2500);
        assert_eq!(m.usage.total(), 15 + 500 + 2500 + 300);
        let ratio = m.usage.cache_hit_ratio().expect("ratio");
        assert!((ratio - 2500.0_f32 / 3015.0_f32).abs() < 1e-4);
    }

    #[test]
    fn usage_missing_yields_zero() {
        let m = parse_session_meta(&fp("minimal.jsonl"));
        assert_eq!(m.usage, Usage::default());
        assert_eq!(m.usage.total(), 0);
        assert_eq!(m.usage.cache_hit_ratio(), None);
    }

    #[test]
    fn title_skips_slash_command_messages() {
        let m = parse_session_meta(&fp("command_first.jsonl"));
        assert!(!m.title.contains("command-name"));
        assert!(m.title.starts_with("Now the real request"));
    }

    #[test]
    fn title_is_whitespace_collapsed() {
        let m = parse_session_meta(&fp("command_first.jsonl"));
        assert!(!m.title.contains('\n'));
        assert!(!m.title.contains("  "));
    }

    #[test]
    fn meta_only_mode_matches_full_mode_for_counts() {
        let full = parse_session(&fp("with_tools.jsonl"));
        let meta = parse_session_meta(&fp("with_tools.jsonl"));
        assert_eq!(meta.tool_count, full.meta.tool_count);
        assert_eq!(meta.message_count, full.meta.message_count);
        assert_eq!(meta.usage, full.meta.usage);
    }

    #[test]
    fn is_noise_empty_string_is_not_noise() {
        assert!(!is_noise(""));
    }

    #[test]
    fn is_noise_with_leading_whitespace() {
        assert!(is_noise("   <command-name>/x</command-name>"));
    }

    #[test]
    fn is_noise_mid_string_is_not_noise() {
        assert!(!is_noise(
            "real text containing <command-name> mid sentence"
        ));
    }

    #[test]
    fn block_serializes_with_kind_discriminator() {
        let b = Block::Text { text: "hi".into() };
        let json = serde_json::to_value(&b).unwrap();
        assert_eq!(json["kind"], "text");
        assert_eq!(json["text"], "hi");
    }

    #[test]
    fn tool_use_serialization_includes_required_fields() {
        let b = Block::ToolUse {
            tool_name: "Bash".into(),
            tool_input: serde_json::json!({"command": "ls"}),
            tool_use_id: "tu-1".into(),
        };
        let json = serde_json::to_value(&b).unwrap();
        assert_eq!(json["kind"], "tool_use");
        assert_eq!(json["tool_name"], "Bash");
        assert_eq!(json["tool_input"]["command"], "ls");
        assert_eq!(json["tool_use_id"], "tu-1");
    }
}
