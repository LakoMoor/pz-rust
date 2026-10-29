use pz_scripts::*;
use std::path::{Path, PathBuf};

fn txt_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() { txt_files(&p, out); } else if p.extension().is_some_and(|x| x == "txt") { out.push(p); }
    }
}

/// Every shipped script parses, and the number of `item` blocks matches a line-based count
/// of `item Name` headers (an independent oracle).
#[test]
fn all_shipped_scripts_parse() {
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let root = PathBuf::from(dir).join("media").join("scripts");
    if !root.is_dir() { return; }
    let mut files = vec![];
    txt_files(&root, &mut files);
    let (mut blocks, mut oracle) = (0usize, 0usize);
    for f in &files {
        let text = String::from_utf8_lossy(&std::fs::read(f).unwrap()).into_owned();
        let entries = parse(&text).unwrap_or_else(|e| panic!("{}: {} at byte {}", f.display(), e.msg, e.offset));
        let mut items = vec![];
        find_all(&entries, "item", &mut items);
        blocks += items.len();
        oracle += text.lines().filter(|l| {
            let mut w = l.split_whitespace();
            w.next() == Some("item") && w.next().is_some_and(|n| n.parse::<f64>().is_err()) && w.next().is_none()
        }).count();
    }
    assert!(files.len() > 500, "files={}", files.len());
    assert_eq!(blocks, oracle, "item block count vs line oracle");
    eprintln!("{} files, {} items", files.len(), blocks);
}
