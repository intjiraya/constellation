use std::path::Path;
use std::time::Instant;

use constellation::index::Index;
use tempfile::TempDir;

const VOCAB: &[&str] = &[
    "authentication",
    "authorization",
    "session",
    "database",
    "migration",
    "performance",
    "logging",
    "metrics",
    "regex",
    "parser",
    "tokenizer",
    "snapshot",
    "rebuild",
    "concurrent",
    "websocket",
    "router",
    "middleware",
    "handler",
    "request",
    "response",
    "search",
    "index",
    "postings",
    "suffix",
    "binary",
    "claude",
    "anthropic",
    "model",
    "prompt",
    "tool",
];

fn synth_body(seed: u64) -> String {
    let mut out = String::with_capacity(4096);
    for i in 0..200 {
        let idx = ((seed.wrapping_mul(31).wrapping_add(i)) as usize) % VOCAB.len();
        out.push_str(VOCAB[idx]);
        out.push(' ');
        if i % 12 == 11 {
            out.push('\n');
        }
    }
    out
}

fn seed_session(project_dir: &Path, sid: &str, body: &str) {
    std::fs::create_dir_all(project_dir).unwrap();
    let escaped: String = body
        .chars()
        .map(|c| match c {
            '\n' => "\\n".to_string(),
            '"' => "\\\"".to_string(),
            '\\' => "\\\\".to_string(),
            c => c.to_string(),
        })
        .collect();
    let content = format!(
        "{{\"type\":\"ai-title\",\"aiTitle\":\"bench\",\"sessionId\":\"{sid}\"}}\n\
{{\"type\":\"user\",\"message\":{{\"role\":\"user\",\"content\":\"{escaped}\"}},\
\"uuid\":\"u-1\",\"timestamp\":\"2026-05-25T11:00:00.000Z\",\"sessionId\":\"{sid}\",\"cwd\":\"/srv/x\"}}\n"
    );
    std::fs::write(project_dir.join(format!("{sid}.jsonl")), content).unwrap();
}

fn rss_kib() -> Option<u64> {
    let txt = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in txt.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let n: u64 = rest.split_whitespace().next()?.parse().ok()?;
            return Some(n);
        }
    }
    None
}

fn median_ns(samples: &mut [u128]) -> u128 {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn p99_ns(samples: &mut [u128]) -> u128 {
    samples.sort_unstable();
    samples[(samples.len() * 99 / 100).min(samples.len() - 1)]
}

fn run(n_sessions: usize, queries: &[&str]) {
    let tmp = TempDir::new().unwrap();

    let seed_start = Instant::now();
    let n_projects = (n_sessions / 10).max(1);
    for i in 0..n_sessions {
        let proj = i % n_projects;
        let proj_dir = tmp.path().join(format!("-bench-{proj:03}"));
        seed_session(&proj_dir, &format!("sess-{i:05}"), &synth_body(i as u64));
    }
    let seed_elapsed = seed_start.elapsed();

    let rss_before = rss_kib();
    let idx = Index::new(tmp.path().to_owned());

    let rebuild_start = Instant::now();
    idx.rebuild();
    let rebuild_total = rebuild_start.elapsed();
    let rss_after = rss_kib();

    let snap = idx.read();
    let projects = snap.projects.len();
    let sessions = snap.by_session_id.len();
    let search_idx = snap.search_index.clone();
    drop(snap);

    println!();
    println!("=== N = {n_sessions} sessions / {n_projects} projects ===");
    println!(
        "seed (synthetic JSONL write): {:>7} ms",
        seed_elapsed.as_millis()
    );
    println!(
        "rebuild() total:              {:>7} ms",
        rebuild_total.as_millis()
    );
    println!("    projects indexed: {projects}, sessions indexed: {sessions}");
    if let (Some(b), Some(a)) = (rss_before, rss_after) {
        println!(
            "RSS delta:                    {:>7} KiB ({} → {})",
            a.saturating_sub(b),
            b,
            a
        );
    }

    for q in queries {
        let terms: Vec<String> = q.split_whitespace().map(str::to_owned).collect();
        let mut samples = Vec::with_capacity(50);
        for _ in 0..50 {
            let t = Instant::now();
            let _hits = search_idx.search(&terms, 50);
            samples.push(t.elapsed().as_nanos());
        }
        let n = search_idx.search(&terms, 50).len();
        let med = median_ns(&mut samples);
        let p99 = p99_ns(&mut samples);
        println!(
            "search {:<20} hits={:>5}  p50 {:>6.2} µs  p99 {:>6.2} µs",
            format!("\"{q}\""),
            n,
            med as f64 / 1000.0,
            p99 as f64 / 1000.0,
        );
    }
}

fn main() {
    println!("constellation-rs search benchmark");
    println!("=================================");
    println!("Note: synthetic data, 1 turn per session, ~4 KiB body each.");

    run(100, &["auth", "session", "model", "auth tool"]);
    run(1_000, &["auth", "session", "model", "auth tool"]);
    run(5_000, &["auth", "session", "model", "auth tool"]);
}
