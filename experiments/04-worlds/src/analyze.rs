//! 本物のプロジェクトを読み取り、世界の形を決める。
//! 規則(docs/concept.md「世界の形の規則」):
//!   上下 = 依存の向き(頼っているものは下) / 水平の距離 = 結びつきの強さ
//!   柱の高さ = 行数(ファイル)、ファイル数(パッケージ)
//! この実験での省略: import は行の文字列から拾っている(本物の構文解析は次の実験)。
//!                   循環参照はまとめていない。Node.js 組み込み(node:fs など)は世界に出していない。

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    File,
    Package,
}

#[derive(Debug)]
pub struct Node {
    pub name: String,
    pub kind: Kind,
    /// ファイルなら行数、パッケージなら中のファイル数
    pub size: usize,
    /// ファイル: 0 以上(何にも頼らないものが 0)。パッケージ: 負(自分のコードからの遠さ)
    pub layer: i32,
    pub deps: Vec<usize>,
    /// 自分で頼んだパッケージか(package.json に書いたか)
    pub direct: bool,
    pub x: f32,
    pub z: f32,
}

pub struct World {
    pub nodes: Vec<Node>,
    pub builtins: usize,
}

fn walk(dir: &Path, skip_modules: bool, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if name.starts_with('.') || (skip_modules && name == "node_modules") {
            continue;
        }
        if p.is_dir() {
            walk(&p, skip_modules, out);
        } else {
            out.push(p);
        }
    }
}

fn is_source(p: &Path) -> bool {
    matches!(p.extension().and_then(|e| e.to_str()), Some("ts" | "tsx" | "js" | "mjs" | "jsx"))
}

/// 1 行から、import / require している相手を拾う
fn specifiers(line: &str) -> Vec<String> {
    let t = line.trim_start();
    if t.starts_with("//") || t.starts_with('*') || t.starts_with("/*") {
        return vec![];
    }
    let mut found = vec![];
    for key in [" from ", "import ", "require(", "import("] {
        let mut rest = t;
        while let Some(i) = rest.find(key) {
            let after = rest[i + key.len()..].trim_start();
            if let Some(q) = after.chars().next().filter(|c| *c == '"' || *c == '\'') {
                if let Some(end) = after[1..].find(q) {
                    found.push(after[1..1 + end].to_string());
                }
            }
            rest = &rest[i + key.len()..];
        }
    }
    found.sort();
    found.dedup();
    found
}

fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

fn package_of(spec: &str) -> String {
    let mut parts = spec.split('/');
    let first = parts.next().unwrap_or("");
    if first.starts_with('@') {
        format!("{}/{}", first, parts.next().unwrap_or(""))
    } else {
        first.to_string()
    }
}

fn json_keys(v: &serde_json::Value, field: &str) -> Vec<String> {
    v.get(field).and_then(|d| d.as_object()).map(|o| o.keys().cloned().collect()).unwrap_or_default()
}

/// 名前から決まる固定の値(0〜1)。でたらめに見えて、毎回同じ
fn hash01(s: &str, salt: u32) -> f32 {
    let mut h: u32 = 2166136261 ^ salt;
    for b in s.bytes() {
        h = (h ^ b as u32).wrapping_mul(16777619);
    }
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b3c6d);
    h ^= h >> 12;
    (h % 100_000) as f32 / 100_000.0
}

pub fn analyze(root: &Path) -> World {
    let mut nodes: Vec<Node> = vec![];
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut add = |nodes: &mut Vec<Node>, name: String, kind: Kind, size: usize| {
        index.insert(name.clone(), nodes.len());
        nodes.push(Node { name, kind, size, layer: 0, deps: vec![], direct: false, x: 0.0, z: 0.0 });
    };

    // 1. 自分のコード
    let mut sources = vec![];
    walk(root, true, &mut sources);
    sources.retain(|p| is_source(p));
    let rel = |p: &Path| p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
    let mut texts = vec![];
    for p in &sources {
        let text = fs::read_to_string(p).unwrap_or_default();
        add(&mut nodes, rel(p), Kind::File, text.lines().count());
        texts.push(text);
    }

    // 2. 依存パッケージ(node_modules の直下と、@名前/ の下)
    let nm = root.join("node_modules");
    let mut pkg_dirs = vec![];
    if let Ok(rd) = fs::read_dir(&nm) {
        let mut top: Vec<_> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        top.sort();
        for p in top {
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if name.starts_with('@') {
                let mut inner: Vec<_> = fs::read_dir(&p).into_iter().flatten().flatten().map(|e| e.path()).collect();
                inner.sort();
                for q in inner.into_iter().filter(|q| q.is_dir()) {
                    pkg_dirs.push((format!("{}/{}", name, q.file_name().unwrap().to_string_lossy()), q));
                }
            } else {
                pkg_dirs.push((name, p));
            }
        }
    }
    let mut manifests = vec![];
    for (name, dir) in &pkg_dirs {
        let mut files = vec![];
        walk(dir, true, &mut files);
        add(&mut nodes, name.clone(), Kind::Package, files.len());
        let json: serde_json::Value =
            fs::read_to_string(dir.join("package.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        manifests.push(json);
    }

    // 3. 線を張る
    let mut builtins = HashSet::new();
    for (i, p) in sources.iter().enumerate() {
        let dir = p.parent().unwrap();
        let mut deps = vec![];
        for line in texts[i].lines() {
            for spec in specifiers(line) {
                if spec.starts_with('.') {
                    let base = normalize(&dir.join(&spec));
                    let tries = ["", ".ts", ".tsx", ".js", "/index.ts", "/index.js"];
                    let hit = tries.iter().map(|t| PathBuf::from(format!("{}{}", base.display(), t))).find(|c| c.is_file());
                    if let Some(j) = hit.and_then(|h| index.get(&rel(&h)).copied()) {
                        deps.push(j);
                    }
                } else if spec.starts_with("node:") {
                    builtins.insert(spec);
                } else if let Some(&j) = index.get(&package_of(&spec)) {
                    deps.push(j);
                }
            }
        }
        deps.sort();
        deps.dedup();
        nodes[i].deps = deps;
    }
    let first_pkg = sources.len();
    for (k, json) in manifests.iter().enumerate() {
        let mut deps: Vec<usize> = ["dependencies", "optionalDependencies"]
            .iter()
            .flat_map(|f| json_keys(json, f))
            .filter_map(|d| index.get(&d).copied())
            .collect();
        deps.sort();
        deps.dedup();
        nodes[first_pkg + k].deps = deps;
    }
    let root_json: serde_json::Value =
        fs::read_to_string(root.join("package.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    for f in ["dependencies", "devDependencies"] {
        for d in json_keys(&root_json, f) {
            if let Some(&j) = index.get(&d) {
                nodes[j].direct = true;
            }
        }
    }

    // 4. 上下: ファイルは、頼っているものより必ず上
    fn file_layer(i: usize, nodes: &[Node], memo: &mut Vec<Option<i32>>, busy: &mut Vec<bool>) -> i32 {
        if let Some(l) = memo[i] {
            return l;
        }
        if busy[i] {
            return 0; // 循環参照(この実験ではまとめていない)
        }
        busy[i] = true;
        let l = nodes[i].deps.iter().filter(|&&d| nodes[d].kind == Kind::File)
            .map(|&d| file_layer(d, nodes, memo, busy) + 1).max().unwrap_or(0);
        busy[i] = false;
        memo[i] = Some(l);
        l
    }
    let mut memo = vec![None; nodes.len()];
    let mut busy = vec![false; nodes.len()];
    for i in 0..first_pkg {
        nodes[i].layer = file_layer(i, &nodes, &mut memo, &mut busy);
    }
    // パッケージは足元。自分のコードからの遠さで、下へ下へ
    let mut dist: Vec<Option<i32>> = vec![None; nodes.len()];
    let mut queue = VecDeque::new();
    for i in 0..nodes.len() {
        let asked = nodes[i].kind == Kind::Package && nodes[i].direct;
        let imported: Vec<usize> = if nodes[i].kind == Kind::File { nodes[i].deps.clone() } else { vec![] };
        for j in imported.into_iter().chain(asked.then_some(i)) {
            if nodes[j].kind == Kind::Package && dist[j].is_none() {
                dist[j] = Some(1);
                queue.push_back(j);
            }
        }
    }
    while let Some(i) = queue.pop_front() {
        for &j in &nodes[i].deps.clone() {
            if dist[j].is_none() {
                dist[j] = Some(dist[i].unwrap() + 1);
                queue.push_back(j);
            }
        }
    }
    let deepest = dist.iter().flatten().copied().max().unwrap_or(0);
    for i in first_pkg..nodes.len() {
        nodes[i].layer = -dist[i].unwrap_or(deepest + 1);
    }

    // 5. 水平: 線でつながるものは引き合い、すべては反発する。始まりの位置は名前から決まる
    let n = nodes.len();
    let mut pos: Vec<(f32, f32)> = nodes.iter().map(|nd| {
        let a = hash01(&nd.name, 1) * std::f32::consts::TAU;
        let r = 1.0 + hash01(&nd.name, 2) * 5.0;
        (r * a.cos(), r * a.sin())
    }).collect();
    let edges: Vec<(usize, usize)> = nodes.iter().enumerate().flat_map(|(i, nd)| nd.deps.iter().map(move |&d| (i, d))).collect();
    for step in 0..500 {
        let cool = 0.12 * (1.0 - step as f32 / 500.0) + 0.01;
        let mut force = vec![(0.0f32, 0.0f32); n];
        for i in 0..n {
            for j in i + 1..n {
                let (dx, dz) = (pos[i].0 - pos[j].0, pos[i].1 - pos[j].1);
                let d2 = (dx * dx + dz * dz).max(0.01);
                let f = 1.6 / d2;
                let d = d2.sqrt();
                force[i].0 += dx / d * f; force[i].1 += dz / d * f;
                force[j].0 -= dx / d * f; force[j].1 -= dz / d * f;
            }
            force[i].0 -= pos[i].0 * 0.05;
            force[i].1 -= pos[i].1 * 0.05;
        }
        for &(a, b) in &edges {
            let (dx, dz) = (pos[b].0 - pos[a].0, pos[b].1 - pos[a].1);
            let d = (dx * dx + dz * dz).sqrt().max(0.01);
            let f = (d - 1.3) * 0.5;
            force[a].0 += dx / d * f; force[a].1 += dz / d * f;
            force[b].0 -= dx / d * f; force[b].1 -= dz / d * f;
        }
        for i in 0..n {
            let (fx, fz) = force[i];
            let m = (fx * fx + fz * fz).sqrt().max(1e-6);
            let s = m.min(4.0) / m * cool;
            pos[i].0 += fx * s;
            pos[i].1 += fz * s;
        }
    }
    for (nd, p) in nodes.iter_mut().zip(pos) {
        nd.x = p.0;
        nd.z = p.1;
    }
    World { nodes, builtins: builtins.len() }
}

/// 読み取った結果を、文字で確かめる
pub fn report(w: &World) {
    let files: Vec<&Node> = w.nodes.iter().filter(|n| n.kind == Kind::File).collect();
    let pkgs: Vec<&Node> = w.nodes.iter().filter(|n| n.kind == Kind::Package).collect();
    println!("自分のコード {} ファイル / 依存パッケージ {} 個(うち自分で頼んだもの {} 個)/ Node.js 組み込み {} 種",
        files.len(), pkgs.len(), pkgs.iter().filter(|p| p.direct).count(), w.builtins);
    let top = files.iter().map(|f| f.layer).max().unwrap_or(0);
    for l in (0..=top).rev() {
        println!("  層 {:>2}:", l);
        for f in files.iter().filter(|f| f.layer == l) {
            let deps: Vec<&str> = f.deps.iter().map(|&d| w.nodes[d].name.as_str()).collect();
            println!("      {:<22} {:>3} 行  → {}", f.name, f.size, if deps.is_empty() { "(何にも頼らない)".into() } else { deps.join(", ") });
        }
    }
    let bottom = pkgs.iter().map(|p| p.layer).min().unwrap_or(0);
    for l in (bottom..0).rev() {
        let names: Vec<String> = pkgs.iter().filter(|p| p.layer == l).map(|p| if p.direct { format!("[{}]", p.name) } else { p.name.clone() }).collect();
        println!("  層 {:>2}: {} 個  {}", l, names.len(), names.join(" "));
    }
}
