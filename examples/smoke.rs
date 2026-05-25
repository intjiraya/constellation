use constellation::scanner::{default_root, scan_projects};

fn main() {
    let root = default_root();
    let t = std::time::Instant::now();
    let projects = scan_projects(&root);
    let dur = t.elapsed();
    let sessions: usize = projects.iter().map(|p| p.sessions.len()).sum();
    println!(
        "scanned {} projects, {} sessions in {:?}",
        projects.len(),
        sessions,
        dur
    );
    println!();
    println!("top 5 by recency:");
    for p in projects.iter().take(5) {
        println!(
            "  {:50}  {:3} sessions  total {:5} msgs",
            p.display_path(),
            p.sessions.len(),
            p.total_messages(),
        );
    }
    println!();
    println!("first project (top), 3 chats:");
    if let Some(p) = projects.first() {
        for s in p.sessions.iter().take(3) {
            let title: String = s.title.chars().take(60).collect();
            println!(
                "  {:8}  msgs={:4}  model={:25}  {}",
                &s.id[..s.id.len().min(8)],
                s.message_count,
                s.model,
                title
            );
        }
    }
}
