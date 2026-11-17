//! 本物のプロジェクトから、入れ子の世界を組み立てる。
//!   世界 → ファイル(柱)・依存パッケージ → 関数 → 文 … と入れ子になり、
//!   中のものは、外のものに対して縮小されている(K)。だから、潜ると中が広大になる。
//! 位置と大きさは倍精度で持つ。深く潜っても、形が崩れないようにするため。

use crate::analyze;
use crate::parse;
use bevy::math::{DVec3, Vec3};
use std::path::Path;

/// 入れ子ひとつぶんの縮小率
const K: f32 = 0.5;
const GAP: f32 = 0.16;
const LAYER_GAP: f32 = 2.6;
const GROUND_GAP: f32 = 1.4;
const PILLAR_WIDTH: f32 = 0.42;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    World,
    File,
    Package,
    Import,
    Fn,
    Call, // 関数を渡す呼び出し。見出しの板の下に、中身が続く
    Stmt,
    Return,
    Type,
    Branch,
    Cond,
    Block,
    Loop,
    Try,
}

pub struct N {
    pub name: String,
    pub kind: Kind,
    /// 自分の中の単位での大きさ
    pub dims: Vec3,
    /// 親の中の単位での位置
    pub pos: Vec3,
    /// 親に対する縮小率
    pub s: f32,
    pub kids: Vec<usize>,
    pub parent: Option<usize>,
    pub depth: usize,
    /// 面を持つ形。中に入ると面が消える
    pub solid: bool,
    /// 線だけの枠(強さ)
    pub frame: f32,
    /// 実行の流れの線(自分の中の単位)
    pub paths: Vec<Vec<Vec3>>,
    /// 世界の中での位置と縮尺(倍精度)
    pub p: DVec3,
    pub sc: f64,
    // 表示の状態
    pub open: f32,
    pub sel_f: f32,
    pub sel: usize,
    pub on_chain: bool,
    pub shown_alpha: f32,
}

pub struct Tree {
    pub n: Vec<N>,
    /// 頼っている相手へ伸びる線(元、先、自分のコードからの線か)
    pub links: Vec<(usize, usize, bool)>,
}

impl Tree {
    fn push(&mut self, name: String, kind: Kind, parent: Option<usize>) -> usize {
        let depth = parent.map_or(0, |p| self.n[p].depth + 1);
        self.n.push(N {
            name, kind, dims: Vec3::ONE, pos: Vec3::ZERO, s: 1.0, kids: vec![], parent, depth,
            solid: false, frame: 0.0, paths: vec![], p: DVec3::ZERO, sc: 1.0,
            open: 0.0, sel_f: 0.0, sel: 0, on_chain: false, shown_alpha: -1.0,
        });
        let i = self.n.len() - 1;
        if let Some(p) = parent {
            self.n[p].kids.push(i);
        }
        i
    }

    pub fn chain(&self, mut i: usize) -> Vec<usize> {
        let mut c = vec![i];
        while let Some(p) = self.n[i].parent {
            c.push(p);
            i = p;
        }
        c.reverse();
        c
    }

    /// 構文解析の木を、世界の木に足す
    fn add(&mut self, pn: &parse::Node, parent: Option<usize>) -> usize {
        use parse::Kind as P;
        let kind = match pn.kind {
            P::File => Kind::File,
            P::Import => Kind::Import,
            P::Fn => Kind::Fn,
            P::Stmt if !pn.kids.is_empty() => Kind::Call,
            P::Stmt => Kind::Stmt,
            P::Return => Kind::Return,
            P::Type => Kind::Type,
            P::Branch => Kind::Branch,
            P::Cond => Kind::Cond,
            P::Block => Kind::Block,
            P::Loop => Kind::Loop,
            P::Try => Kind::Try,
        };
        let i = self.push(pn.label.clone(), kind, parent);
        if kind == Kind::Call {
            let h = self.push(pn.label.clone(), Kind::Stmt, Some(i));
            self.n[h].dims = Vec3::new(1.0, 0.22, 1.0);
            self.n[h].solid = true;
        }
        for k in &pn.kids {
            self.add(k, Some(i));
        }
        let slab = 0.12 + 0.1 * pn.lines as f32;
        let leaf = self.n[i].kids.is_empty();
        match kind {
            Kind::Import => self.leaf(i, Vec3::new(1.0, 0.1, 1.0)),
            Kind::Cond => self.leaf(i, Vec3::new(0.7, 0.2, 0.7)),
            Kind::Type => {
                self.n[i].dims = Vec3::new(1.0, slab, 1.0);
                self.n[i].frame = 0.6;
            }
            Kind::Stmt | Kind::Return => self.leaf(i, Vec3::new(1.0, slab, 1.0)),
            Kind::File if leaf => self.leaf(i, Vec3::new(1.0, slab.max(0.6), 1.0)),
            _ if leaf => self.leaf(i, Vec3::new(1.0, slab, 1.0)),
            Kind::File => {
                self.stack(i, 0.3);
                self.n[i].solid = true;
            }
            Kind::Fn => {
                self.stack(i, 0.2);
                self.n[i].frame = 0.28;
            }
            Kind::Block | Kind::Call => self.stack(i, 0.0),
            Kind::Branch | Kind::Try => self.split(i),
            Kind::Loop => self.ring(i),
            Kind::World | Kind::Package => {}
        }
        i
    }

    fn leaf(&mut self, i: usize, dims: Vec3) {
        self.n[i].dims = dims;
        self.n[i].solid = true;
    }

    fn kid_sizes(&self, i: usize) -> Vec<(usize, Vec3)> {
        self.n[i].kids.iter().map(|&k| (k, self.n[k].dims * K)).collect()
    }

    fn place(&mut self, k: usize, pos: Vec3) {
        self.n[k].pos = pos;
        self.n[k].s = K;
    }

    /// 上から下へ、書かれた順に積む
    fn stack(&mut self, i: usize, pad: f32) {
        let ks = self.kid_sizes(i);
        let w = ks.iter().map(|k| k.1.x).fold(0.55, f32::max);
        let many = ks.len() > 1;
        let d = ks.iter().map(|k| k.1.z).fold(0.55, f32::max) + if many { 0.12 } else { 0.0 };
        let h = ks.iter().map(|k| k.1.y).sum::<f32>() + GAP * ks.len().saturating_sub(1) as f32 + pad * 2.0;
        let mut y = h / 2.0 - pad;
        let mut flow = vec![Vec3::new(0.0, h / 2.0, 0.0)];
        for (j, (k, size)) in ks.into_iter().enumerate() {
            let z = if many { if j % 2 == 0 { -0.06 } else { 0.06 } } else { 0.0 };
            let c = Vec3::new(0.0, y - size.y / 2.0, z);
            y -= size.y + GAP;
            flow.push(c);
            self.place(k, c);
        }
        flow.push(Vec3::new(0.0, -h / 2.0, 0.0));
        self.n[i].dims = Vec3::new(w, h, d);
        self.n[i].paths = vec![flow];
    }

    /// 条件の下で、積層が横に分かれて、また一本に戻る
    fn split(&mut self, i: usize) {
        let mut ks = self.kid_sizes(i);
        let cond = if self.n[i].kind == Kind::Branch { Some(ks.remove(0)) } else { None };
        let col_gap = 0.25;
        let only_one = self.n[i].kind == Kind::Branch && ks.len() == 1;
        let cols_w = ks.iter().map(|k| k.1.x).sum::<f32>() + col_gap * ks.len().saturating_sub(1) as f32;
        let w = if only_one { cols_w + col_gap + 0.3 } else { cols_w };
        let col_h = ks.iter().map(|k| k.1.y).fold(0.0, f32::max);
        let cond_h = cond.map_or(0.0, |c| c.1.y);
        let h = cond_h + 0.2 + col_h + 0.18;
        let d = ks.iter().map(|k| k.1.z).fold(0.5, f32::max);
        let (top, bottom) = (Vec3::new(0.0, h / 2.0, 0.0), Vec3::new(0.0, -h / 2.0, 0.0));
        let fork = Vec3::new(0.0, h / 2.0 - cond_h, 0.0);
        let col_top = h / 2.0 - cond_h - 0.2;
        let mut paths = vec![vec![top, fork]];
        if let Some((c, _)) = cond {
            self.place(c, Vec3::new(0.0, h / 2.0 - cond_h / 2.0, 0.0));
        }
        let mut x = -w / 2.0;
        for (k, size) in ks {
            let cx = x + size.x / 2.0;
            x += size.x + col_gap;
            self.place(k, Vec3::new(cx, col_top - size.y / 2.0, 0.0));
            paths.push(vec![fork, Vec3::new(cx, col_top, 0.0), Vec3::new(cx, -h / 2.0 + 0.07, 0.0), bottom]);
        }
        if only_one {
            // else がない: 何もしない側の線だけが通る
            let cx = w / 2.0 - 0.15;
            paths.push(vec![fork, Vec3::new(cx, col_top, 0.0), Vec3::new(cx, -h / 2.0 + 0.07, 0.0), bottom]);
        }
        self.n[i].dims = Vec3::new(w.max(0.5), h, d);
        self.n[i].paths = paths;
    }

    /// 中身が輪になる
    fn ring(&mut self, i: usize) {
        let ks = self.kid_sizes(i);
        let around: f32 = ks.iter().map(|k| k.1.x.max(k.1.z) + 0.3).sum();
        let r = (around / std::f32::consts::TAU).max(0.5);
        let kid_w = ks.iter().map(|k| k.1.x.max(k.1.z)).fold(0.5, f32::max);
        let h = ks.iter().map(|k| k.1.y).fold(0.2, f32::max) + 0.3;
        let at = |t: f32| Vec3::new(r * t.sin(), 0.0, r * t.cos());
        let count = ks.len().max(1) as f32;
        for (j, (k, _)) in ks.into_iter().enumerate() {
            self.place(k, at((j as f32 + 0.5) / count * std::f32::consts::TAU));
        }
        let circle: Vec<Vec3> = (0..=96).map(|j| at(j as f32 / 96.0 * std::f32::consts::TAU)).collect();
        self.n[i].paths = vec![
            circle,
            vec![Vec3::new(0.0, h / 2.0, 0.0), Vec3::new(0.0, h / 2.0, r), at(0.0)],
            vec![at(0.0), Vec3::new(0.0, -h / 2.0, r), Vec3::new(0.0, -h / 2.0, 0.0)],
        ];
        let side = 2.0 * r + kid_w;
        self.n[i].dims = Vec3::new(side, h, side);
    }

    fn settle(&mut self, i: usize) {
        if let Some(p) = self.n[i].parent {
            self.n[i].sc = self.n[p].sc * self.n[i].s as f64;
            self.n[i].p = self.n[p].p + self.n[i].pos.as_dvec3() * self.n[p].sc;
        }
        for k in self.n[i].kids.clone() {
            self.settle(k);
        }
    }
}

pub fn build(root: &Path) -> Tree {
    let a = analyze::analyze(root);
    analyze::report(&a);
    let mut t = Tree { n: vec![], links: vec![] };
    let world = t.push(format!("{}/", root.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "world".into())), Kind::World, None);

    let files: Vec<&analyze::Node> = a.nodes.iter().filter(|n| n.kind == analyze::Kind::File).collect();
    let cx = files.iter().map(|n| n.x).sum::<f32>() / files.len().max(1) as f32;
    let cz = files.iter().map(|n| n.z).sum::<f32>() / files.len().max(1) as f32;

    let mut index = vec![0usize; a.nodes.len()];
    for (ai, an) in a.nodes.iter().enumerate() {
        let i = match an.kind {
            analyze::Kind::File => {
                let i = match parse::parse_file(&root.join(&an.name), an.name.clone()) {
                    Ok(pn) => t.add(&pn, Some(world)),
                    Err(e) => {
                        eprintln!("{e}");
                        let i = t.push(an.name.clone(), Kind::File, Some(world));
                        t.leaf(i, Vec3::new(1.0, 0.3 + an.size as f32 * 0.1, 1.0));
                        i
                    }
                };
                // 柱の太さは揃える。高さは、中身の量で決まる
                t.n[i].s = PILLAR_WIDTH / t.n[i].dims.x;
                t.n[i].pos = Vec3::new(an.x - cx, an.layer as f32 * LAYER_GAP, an.z - cz);
                i
            }
            analyze::Kind::Package => {
                let i = t.push(an.name.clone(), Kind::Package, Some(world));
                let f = (an.size as f32 + 1.0).ln(); // 大きさ = 中のファイル数(対数)
                t.leaf(i, Vec3::new(0.25 + f * 0.09, 0.2 + f * 0.33, 0.25 + f * 0.09));
                t.n[i].pos = Vec3::new(an.x - cx, -GROUND_GAP + an.layer as f32 * LAYER_GAP, an.z - cz);
                i
            }
        };
        index[ai] = i;
    }
    for (ai, an) in a.nodes.iter().enumerate() {
        for &d in &an.deps {
            t.links.push((index[ai], index[d], an.kind == analyze::Kind::File));
        }
    }
    // 選ぶ順: 上の層から。自分のコードが先、依存パッケージが後
    let mut order = t.n[world].kids.clone();
    order.sort_by(|&x, &y| {
        let key = |i: usize| ((t.n[i].kind == Kind::Package) as i32, -(t.n[i].pos.y * 100.0) as i32, (t.n[i].pos.x * 100.0) as i32);
        key(x).cmp(&key(y))
    });
    t.n[world].kids = order;
    t.n[world].dims = Vec3::splat(12.0);
    t.settle(world);
    t
}
