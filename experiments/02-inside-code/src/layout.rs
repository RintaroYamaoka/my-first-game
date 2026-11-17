//! 木から、形と位置を決める。規則は種類ごとに違う(docs/concept.md「世界の形の規則」)。
//!   文 = 板。厚みは、その文の行数
//!   関数・ブロック・ファイル = 板が、書かれた順に上から下へ積み重なる
//!   分岐 = 条件の下で、積層が横に分かれて、また一本に戻る
//!   ループ = 中身が輪になる
//!   型 = 線だけの枠
//! この実験では、入れ子を縮小していない(潜るための拡大は、まだ入れていない)。

use crate::parse::{Kind, Node};
use bevy::math::Vec3;

const GAP: f32 = 0.16;

pub struct Item {
    pub kind: Kind,
    pub label: String,
    pub center: Vec3,
    pub size: Vec3,
    /// 面を持つか(板)。持たないものは、線だけの枠
    pub solid: bool,
}

#[derive(Default)]
pub struct Scene {
    pub items: Vec<Item>,
    /// 実行の流れを表す線
    pub flows: Vec<Vec<Vec3>>,
}

struct Laid<'a> {
    node: &'a Node,
    size: Vec3,
    kids: Vec<(Vec3, Laid<'a>)>,
    /// 自分の中心から見た、流れの線
    flows: Vec<Vec<Vec3>>,
    /// 子を持つ文の、見出しの板
    header: Option<(Vec3, Vec3)>,
}

fn slab(lines: usize) -> f32 {
    0.12 + 0.1 * lines as f32
}

fn lay(n: &Node) -> Laid<'_> {
    let leaf = |size: Vec3| Laid { node: n, size, kids: vec![], flows: vec![], header: None };
    match n.kind {
        Kind::Import => leaf(Vec3::new(1.0, 0.1, 1.0)),
        Kind::Cond => leaf(Vec3::new(0.7, 0.2, 0.7)),
        Kind::Return | Kind::Type => leaf(Vec3::new(1.0, slab(n.lines), 1.0)),
        Kind::Stmt if n.kids.is_empty() => leaf(Vec3::new(1.0, slab(n.lines), 1.0)),
        Kind::Fn if n.kids.is_empty() => leaf(Vec3::new(1.0, slab(n.lines), 1.0)),
        Kind::File | Kind::Fn | Kind::Block | Kind::Stmt => stack(n),
        Kind::Branch | Kind::Try => split(n),
        Kind::Loop => ring(n),
    }
}

/// 上から下へ、書かれた順に積む
fn stack(n: &Node) -> Laid<'_> {
    let pad = match n.kind { Kind::File => 0.3, Kind::Fn => 0.2, _ => 0.0 };
    let kids: Vec<Laid> = n.kids.iter().map(lay).collect();
    // 子を持つ文(コールバックを渡す呼び出し)は、まず自分の板があり、その下に中身が続く
    let head = if n.kind == Kind::Stmt { Some(Vec3::new(1.0, 0.22, 1.0)) } else { None };
    let w = kids.iter().map(|k| k.size.x).fold(1.0, f32::max);
    let d = kids.iter().map(|k| k.size.z).fold(1.0, f32::max) + if kids.len() > 1 { 0.24 } else { 0.0 };
    let count = kids.len() + head.is_some() as usize;
    let h = kids.iter().map(|k| k.size.y).sum::<f32>() + head.map_or(0.0, |s| s.y) + GAP * count.saturating_sub(1) as f32 + pad * 2.0;
    let mut y = h / 2.0 - pad;
    let mut flow = vec![Vec3::new(0.0, h / 2.0, 0.0)];
    let header = head.map(|s| {
        let c = Vec3::new(0.0, y - s.y / 2.0, 0.0);
        y -= s.y + GAP;
        flow.push(c);
        (c, s)
    });
    let many = kids.len() > 1;
    let mut placed = vec![];
    for (i, k) in kids.into_iter().enumerate() {
        let z = if many { if i % 2 == 0 { -0.12 } else { 0.12 } } else { 0.0 };
        let c = Vec3::new(0.0, y - k.size.y / 2.0, z);
        y -= k.size.y + GAP;
        flow.push(c);
        placed.push((c, k));
    }
    flow.push(Vec3::new(0.0, -h / 2.0, 0.0));
    Laid { node: n, size: Vec3::new(w, h, d), kids: placed, flows: vec![flow], header }
}

/// 条件の下で、積層が横に分かれて、また一本に戻る
fn split(n: &Node) -> Laid<'_> {
    let mut kids: Vec<Laid> = n.kids.iter().map(lay).collect();
    let cond = if n.kind == Kind::Branch { Some(kids.remove(0)) } else { None };
    let col_gap = 0.4;
    let w = kids.iter().map(|k| k.size.x).sum::<f32>() + col_gap * kids.len().saturating_sub(1) as f32;
    let col_h = kids.iter().map(|k| k.size.y).fold(0.0, f32::max);
    let cond_h = cond.as_ref().map_or(0.0, |c| c.size.y);
    let h = cond_h + 0.3 + col_h + 0.25;
    let d = kids.iter().map(|k| k.size.z).fold(1.0, f32::max);
    let top = Vec3::new(0.0, h / 2.0, 0.0);
    let bottom = Vec3::new(0.0, -h / 2.0, 0.0);
    let fork = Vec3::new(0.0, h / 2.0 - cond_h, 0.0);
    let col_top = h / 2.0 - cond_h - 0.3;
    let mut placed = vec![];
    let mut flows = vec![vec![top, fork]];
    if let Some(c) = cond {
        placed.push((Vec3::new(0.0, h / 2.0 - cond_h / 2.0, 0.0), c));
    }
    // 分岐が片方だけ(else がない)なら、何もしない側の線も引く
    let only_one = n.kind == Kind::Branch && kids.len() == 1;
    let total_w = if only_one { w + col_gap + 0.5 } else { w };
    let mut x = -total_w / 2.0;
    for k in kids {
        let cx = x + k.size.x / 2.0;
        x += k.size.x + col_gap;
        let c = Vec3::new(cx, col_top - k.size.y / 2.0, 0.0);
        flows.push(vec![fork, Vec3::new(cx, col_top, 0.0), Vec3::new(cx, col_top - k.size.y, 0.0), Vec3::new(cx, -h / 2.0 + 0.1, 0.0), bottom]);
        placed.push((c, k));
    }
    if only_one {
        let cx = total_w / 2.0 - 0.25;
        flows.push(vec![fork, Vec3::new(cx, col_top, 0.0), Vec3::new(cx, -h / 2.0 + 0.1, 0.0), bottom]);
    }
    Laid { node: n, size: Vec3::new(total_w.max(1.0), h, d), kids: placed, flows, header: None }
}

/// 中身が輪になる
fn ring(n: &Node) -> Laid<'_> {
    let kids: Vec<Laid> = n.kids.iter().map(lay).collect();
    let around: f32 = kids.iter().map(|k| k.size.x.max(k.size.z) + 0.5).sum();
    let r = (around / std::f32::consts::TAU).max(0.95);
    let kid_w = kids.iter().map(|k| k.size.x.max(k.size.z)).fold(1.0, f32::max);
    let kid_h = kids.iter().map(|k| k.size.y).fold(0.3, f32::max);
    let h = kid_h + 0.5;
    let at = |t: f32| Vec3::new(r * t.sin(), 0.0, r * t.cos());
    let count = kids.len().max(1) as f32;
    // 入口は手前。そこから一周して、同じ場所に戻る
    let placed: Vec<(Vec3, Laid)> = kids.into_iter().enumerate()
        .map(|(i, k)| (at((i as f32 + 0.5) / count * std::f32::consts::TAU), k)).collect();
    let circle: Vec<Vec3> = (0..=96).map(|i| at(i as f32 / 96.0 * std::f32::consts::TAU)).collect();
    let flows = vec![
        circle,
        vec![Vec3::new(0.0, h / 2.0, 0.0), Vec3::new(0.0, h / 2.0, r), at(0.0)],
        vec![at(0.0), Vec3::new(0.0, -h / 2.0, r), Vec3::new(0.0, -h / 2.0, 0.0)],
    ];
    let side = 2.0 * r + kid_w;
    Laid { node: n, size: Vec3::new(side, h, side), kids: placed, flows, header: None }
}

fn flatten(l: &Laid, origin: Vec3, out: &mut Scene) {
    let n = l.node;
    let leaf = l.kids.is_empty() && l.header.is_none();
    let frame = matches!(n.kind, Kind::File | Kind::Fn) && !leaf;
    if leaf {
        out.items.push(Item { kind: n.kind, label: n.label.clone(), center: origin, size: l.size, solid: n.kind != Kind::Type });
    } else if frame || n.kind == Kind::Loop {
        out.items.push(Item { kind: n.kind, label: n.label.clone(), center: origin, size: l.size, solid: false });
    }
    if let Some((c, s)) = l.header {
        out.items.push(Item { kind: Kind::Stmt, label: n.label.clone(), center: origin + c, size: s, solid: true });
    }
    for f in &l.flows {
        out.flows.push(f.iter().map(|p| origin + *p).collect());
    }
    for (c, k) in &l.kids {
        flatten(k, origin + *c, out);
    }
}

/// ファイルを横に並べる。天辺を揃えるので、上から下へ読める
pub fn build(files: &[Node]) -> Scene {
    let laid: Vec<Laid> = files.iter().map(lay).collect();
    let gap = 1.6;
    let total: f32 = laid.iter().map(|l| l.size.x).sum::<f32>() + gap * laid.len().saturating_sub(1) as f32;
    let mut x = -total / 2.0;
    let mut scene = Scene::default();
    for l in &laid {
        flatten(l, Vec3::new(x + l.size.x / 2.0, -l.size.y / 2.0, 0.0), &mut scene);
        x += l.size.x + gap;
    }
    scene
}
