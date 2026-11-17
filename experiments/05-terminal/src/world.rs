//! 本物のプロジェクトから、「世界の集まり」を組み立てる。
//!   中身を持つものは、それぞれが一つの世界を持つ。世界は、外から見た形の中には入っていない。
//!   外から見た形は入口の姿で、潜った先は、その世界だけの広さと法則を持つ。
//! 法則(世界の種類ごとに、並べ方が違う):
//!   Pillars … プロジェクト。柱が並ぶ。上下 = 依存の向き
//!   Strata  … ファイル。書かれた順に、下へ降りていく地層
//!   Road    … 関数。実行の順に前へ進む道。分岐で道が分かれ、return は道をふさぐ壁
//!   Ring    … ループ。道が輪になって、元の場所に戻る

use crate::analyze;
use crate::parse;
use bevy::math::Vec3;
use std::f32::consts::{FRAC_PI_2, TAU};
use std::path::Path;

const ROAD_W: f32 = 16.0;
const LANE_GAP: f32 = 9.0;
const GAP: f32 = 7.0;
const FORK: f32 = 16.0;
const FLOW_Y: f32 = 0.38;
const GATE_H: f32 = 11.0;
const POST: f32 = 1.3;
const EYE: f32 = 2.6;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Law {
    Pillars,
    Strata,
    Road,
    Ring,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    World,
    File,
    Package,
    Import,
    Fn,
    Call, // 関数を渡す呼び出し
    Stmt,
    Return,
    Type,
    Branch,
    Cond,
    Block,
    Loop,
    Try,
}

/// 世界の中に置かれた形ひとつ
pub struct Item {
    pub node: usize,
    pub pos: Vec3,
    pub dims: Vec3,
    pub yaw: f32,
    /// 面を持つ形か(型は、線だけの枠)
    pub solid: bool,
    /// 中の区切りを、外から見える帯として出す(上からの割合)
    pub bands: Vec<f32>,
    /// 中から戻ってきたときに立つ場所(なければ、形の手前)
    pub door: Option<Vec3>,
    /// 形を作る箱(自分の中での位置、大きさ)。空なら、全体が一つの箱
    pub parts: Vec<(Vec3, Vec3)>,
}

pub struct World {
    pub law: Law,
    pub items: Vec<Item>,
    /// 実行の流れの線
    pub paths: Vec<Vec<Vec3>>,
    /// 流れの線のうち、道がないもの(何もしない側)
    pub thin: Vec<Vec<Vec3>>,
    /// 道(位置、大きさ、向き)
    pub decor: Vec<(Vec3, Vec3, f32)>,
    /// 頼っている相手へ伸びる線(元、先、自分のコードからの線か)。Item の番号
    pub links: Vec<(usize, usize, bool)>,
    /// 潜ってきたときに立つ場所と向き
    pub spawn: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    /// 世界の広さ(霞の濃さを決める)
    pub radius: f32,
}

impl World {
    fn new(law: Law) -> Self {
        World { law, items: vec![], paths: vec![], thin: vec![], decor: vec![], links: vec![], spawn: Vec3::ZERO, yaw: 0.0, pitch: 0.0, radius: 60.0 }
    }

    fn stand(&mut self, spawn: Vec3, target: Vec3) {
        self.spawn = spawn;
        (self.yaw, self.pitch) = look(spawn, target);
        let far = self.items.iter().map(|it| it.pos.distance(spawn) + it.dims.max_element() * 0.5).fold(0.0, f32::max);
        self.radius = far.max(60.0);
    }
}

/// from から to を見る向き(ヨー、ピッチ)。前は -Z
pub fn look(from: Vec3, to: Vec3) -> (f32, f32) {
    let d = (to - from).normalize_or(Vec3::NEG_Z);
    ((-d.x).atan2(-d.z), d.y.asin())
}

pub struct N {
    pub name: String,
    pub kind: Kind,
    pub lines: usize,
    pub kids: Vec<usize>,
    /// 中身を持つものは、自分の世界を持つ
    pub world: Option<World>,
    /// 自分が置かれている世界(その世界を持つ節、Item の番号)
    pub owner: Option<(usize, usize)>,
}

pub struct Tree {
    pub n: Vec<N>,
}

fn line(x: f32, z0: f32, z1: f32) -> Vec<Vec3> {
    vec![Vec3::new(x, FLOW_Y, z0), Vec3::new(x, FLOW_Y, z1)]
}

impl Tree {
    fn push(&mut self, name: String, kind: Kind, lines: usize, parent: Option<usize>) -> usize {
        self.n.push(N { name, kind, lines, kids: vec![], world: None, owner: None });
        let i = self.n.len() - 1;
        if let Some(p) = parent {
            self.n[p].kids.push(i);
        }
        i
    }

    /// 構文解析の木を、そのまま写す
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
        let i = self.push(pn.label.clone(), kind, pn.lines, parent);
        for k in &pn.kids {
            self.add(k, Some(i));
        }
        i
    }

    /// 世界の持ち主の名前を、外から順に
    pub fn chain(&self, mut i: usize) -> Vec<usize> {
        let mut c = vec![i];
        while let Some((o, _)) = self.n[i].owner {
            c.push(o);
            i = o;
        }
        c.reverse();
        c
    }

    fn bands(&self, k: usize) -> Vec<f32> {
        let kids = &self.n[k].kids;
        let total: f32 = kids.iter().map(|&c| self.n[c].lines as f32).sum();
        let mut at = 0.0;
        kids.iter().take(kids.len().saturating_sub(1)).map(|&c| {
            at += self.n[c].lines as f32;
            at / total.max(1.0)
        }).collect()
    }

    /// 形を世界に置く。中身を持つものなら、その中の世界も作る
    fn item(&mut self, w: &mut World, owner: usize, k: usize, pos: Vec3, dims: Vec3) {
        let enter = !self.n[k].kids.is_empty();
        if enter && self.n[k].world.is_none() {
            self.make_world(k);
        }
        self.n[k].owner = Some((owner, w.items.len()));
        let bands = if enter { self.bands(k) } else { vec![] };
        w.items.push(Item { node: k, pos, dims, yaw: 0.0, solid: self.n[k].kind != Kind::Type, bands, door: None, parts: vec![] });
    }

    fn make_world(&mut self, i: usize) {
        let w = match self.n[i].kind {
            Kind::File => self.strata(i),
            Kind::Loop => self.ring(i),
            _ => self.road(i),
        };
        self.n[i].world = Some(w);
    }

    /// ファイルの世界: 書かれた順に、下へ降りていく段丘。段の高さ = 行数。段は左右と奥へ、果てなく続く
    fn strata(&mut self, i: usize) -> World {
        const TREAD: f32 = 13.0;
        const BACK: f32 = -900.0;
        let mut w = World::new(Law::Strata);
        let (mut top, mut front) = (0.0f32, 0.0f32);
        let mut flow = vec![];
        for k in self.n[i].kids.clone() {
            let th = 1.5 + self.n[k].lines as f32 * 1.1;
            flow.push(Vec3::new(0.0, top + 0.06, front - TREAD));
            flow.push(Vec3::new(0.0, top + 0.06, front));
            self.item(&mut w, i, k, Vec3::new(0.0, top - th / 2.0, (front + BACK) / 2.0), Vec3::new(1800.0, th, front - BACK));
            w.items.last_mut().unwrap().door = Some(Vec3::new(-6.0, top + 5.0, front + 16.0));
            top -= th;
            front += TREAD;
        }
        flow.push(Vec3::new(0.0, top + 0.06, front - TREAD));
        w.paths.push(flow);
        w.stand(Vec3::new(-20.0, top * 0.25 + 9.0, front + 26.0), Vec3::new(4.0, top * 0.55, front * 0.5));
        w.radius = (-top + 150.0).max(170.0);
        w
    }

    /// 流れの線に沿って、道を敷く
    fn pave(w: &mut World) {
        for path in &w.paths {
            for pair in path.windows(2) {
                let d = pair[1] - pair[0];
                if d.length() > 0.01 {
                    let mid = (pair[0] + pair[1]) * 0.5;
                    w.decor.push((Vec3::new(mid.x, 0.0, mid.z), Vec3::new(ROAD_W, 0.6, d.length() + 0.4), (-d.x).atan2(-d.z)));
                }
            }
        }
    }

    /// 関数の世界: 実行の順に前へ進む道。文は、くぐっていく門
    fn road(&mut self, i: usize) -> World {
        let mut w = World::new(Law::Road);
        let seq = match self.n[i].kind {
            Kind::Branch | Kind::Try => vec![i],
            _ => self.n[i].kids.clone(),
        };
        w.paths.push(line(0.0, 18.0, 0.0));
        let end = self.flow(&mut w, i, &seq, 0.0, 0.0);
        w.paths.push(line(0.0, end, end - 400.0)); // 道は、霞の向こうへ続く
        Self::pave(&mut w);
        w.stand(Vec3::new(-2.0, EYE, 15.0), Vec3::new(0.0, EYE + 1.0, -40.0));
        w.radius = (-end * 0.8).clamp(150.0, 400.0);
        w
    }

    /// ループの世界: 道が輪になる。進むと、元の場所に戻る
    fn ring(&mut self, i: usize) -> World {
        let mut w = World::new(Law::Ring);
        let kids = self.n[i].kids.clone();
        let end = self.flow(&mut w, i, &kids, 0.0, 0.0);
        let len = (-end + GAP).max(190.0);
        let r = len / TAU;
        let angle = |z: f32| -z / len * TAU;
        let bend = |p: Vec3| {
            let a = angle(p.z);
            Vec3::new((r + p.x) * a.sin(), p.y, (r + p.x) * a.cos())
        };
        for it in &mut w.items {
            it.yaw = angle(it.pos.z) - FRAC_PI_2;
            it.pos = bend(it.pos);
        }
        // 最後の文から、最初の文へ戻る
        w.paths.push(line(0.0, end, -len));
        let curve = |paths: &Vec<Vec<Vec3>>| -> Vec<Vec<Vec3>> {
            paths.iter().map(|path| {
                let mut out = vec![];
                for pair in path.windows(2) {
                    let steps = (pair[0].distance(pair[1]) / 3.0).ceil().max(1.0) as usize;
                    for s in 0..=steps {
                        out.push(bend(pair[0].lerp(pair[1], s as f32 / steps as f32)));
                    }
                }
                out
            }).collect()
        };
        w.paths = curve(&w.paths);
        w.thin = curve(&w.thin);
        Self::pave(&mut w);
        let spawn = bend(Vec3::new(-2.0, EYE, 14.0));
        w.stand(spawn, bend(Vec3::new(0.0, EYE + 1.0, -30.0)));
        w.radius = (r * 2.6).max(150.0);
        w
    }

    fn width(&self, k: usize) -> f32 {
        let kids = &self.n[k].kids;
        match self.n[k].kind {
            Kind::Block => kids.iter().map(|&c| self.width(c)).fold(ROAD_W, f32::max),
            Kind::Branch | Kind::Try => {
                let lanes: Vec<f32> = kids.iter().filter(|&&c| self.n[c].kind != Kind::Cond).map(|&c| self.width(c)).collect();
                let bypass = if self.n[k].kind == Kind::Branch && lanes.len() == 1 { LANE_GAP + 3.0 } else { 0.0 };
                (lanes.iter().sum::<f32>() + LANE_GAP * lanes.len().saturating_sub(1) as f32 + bypass).max(ROAD_W)
            }
            _ => ROAD_W,
        }
    }

    /// 門: 道をまたぐ枠。奥行き = 行数
    fn gate(&mut self, w: &mut World, owner: usize, k: usize, x: f32, z: f32, h: f32, depth: f32) {
        let wide = ROAD_W + POST * 2.0;
        self.item(w, owner, k, Vec3::new(x, h / 2.0 + 0.3, z - depth / 2.0), Vec3::new(wide, h, depth));
        let side = (wide - POST) / 2.0;
        w.items.last_mut().unwrap().parts = vec![
            (Vec3::new(-side, 0.0, 0.0), Vec3::new(POST, h, depth)),
            (Vec3::new(side, 0.0, 0.0), Vec3::new(POST, h, depth)),
            (Vec3::new(0.0, (h - POST) / 2.0, 0.0), Vec3::new(wide - POST * 2.0, POST, depth)),
        ];
    }

    /// 文を、前(-Z)へ順に並べる。終わりの位置を返す
    fn flow(&mut self, w: &mut World, owner: usize, seq: &[usize], x: f32, z0: f32) -> f32 {
        let (mut z, mut seg) = (z0, z0);
        for &k in seq {
            let kind = self.n[k].kind;
            let kids = self.n[k].kids.clone();
            match kind {
                Kind::Block => {
                    w.paths.push(line(x, seg, z));
                    z = self.flow(w, owner, &kids, x, z);
                    seg = z;
                }
                Kind::Branch | Kind::Try => {
                    let mut lanes = kids;
                    if kind == Kind::Branch && !lanes.is_empty() {
                        // 条件: 道が分かれる手前の、低い門
                        let c = lanes.remove(0);
                        self.gate(w, owner, c, x, z, GATE_H * 0.55, 1.6);
                        z -= 1.6 + 2.0;
                    }
                    w.paths.push(line(x, seg, z));
                    let (fork, start) = (z, z - FORK);
                    let widths: Vec<f32> = lanes.iter().map(|&l| self.width(l)).collect();
                    let bypass = kind == Kind::Branch && lanes.len() == 1;
                    let total = widths.iter().sum::<f32>() + LANE_GAP * lanes.len().saturating_sub(1) as f32 + if bypass { LANE_GAP + 3.0 } else { 0.0 };
                    let mut lx = x - total / 2.0;
                    let mut ends = vec![];
                    for (&l, &wd) in lanes.iter().zip(&widths) {
                        let cx = lx + wd / 2.0;
                        lx += wd + LANE_GAP;
                        ends.push((cx, self.flow(w, owner, &[l], cx, start)));
                    }
                    let merge = ends.iter().map(|e| e.1).fold(start, f32::min) - FORK;
                    for (cx, e) in ends {
                        w.paths.push(vec![Vec3::new(x, FLOW_Y, fork), Vec3::new(cx, FLOW_Y, start)]);
                        w.paths.push(vec![Vec3::new(cx, FLOW_Y, e), Vec3::new(cx, FLOW_Y, merge + FORK), Vec3::new(x, FLOW_Y, merge)]);
                    }
                    if bypass {
                        // else がない: 何もしない側は、道がなく、線だけが通る
                        let cx = x + total / 2.0 - 1.5;
                        w.thin.push(vec![Vec3::new(x, FLOW_Y, fork), Vec3::new(cx, FLOW_Y, start), Vec3::new(cx, FLOW_Y, merge + FORK), Vec3::new(x, FLOW_Y, merge)]);
                    }
                    z = merge;
                    seg = merge;
                }
                Kind::Return => {
                    // return: 道をふさぐ壁
                    self.item(w, owner, k, Vec3::new(x, GATE_H / 2.0 + 0.3, z - 0.7), Vec3::new(ROAD_W + POST * 2.0, GATE_H, 1.4));
                    z -= 1.4 + GAP;
                }
                _ => {
                    let lines = self.n[k].lines as f32;
                    if kids.is_empty() {
                        // ただの文: くぐる門。奥行き = 行数
                        let depth = lines * 2.4;
                        self.gate(w, owner, k, x, z, GATE_H, depth);
                        z -= depth + GAP;
                    } else {
                        // 中に世界を持つもの: 道の真ん中に立つ塊。高さ = 行数
                        let (h, depth) = (8.0 + lines * 1.5, 5.0 + lines * 1.2);
                        self.item(w, owner, k, Vec3::new(x, h / 2.0 + 0.3, z - depth / 2.0), Vec3::new(ROAD_W * 0.55, h, depth));
                        z -= depth + GAP;
                    }
                }
            }
        }
        w.paths.push(line(x, seg, z));
        z
    }
}

pub fn build(root: &Path) -> Tree {
    let a = analyze::analyze(root);
    analyze::report(&a);
    let mut t = Tree { n: vec![] };
    let name = root.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "world".into());
    let world = t.push(format!("{name}/"), Kind::World, 0, None);

    // プロジェクトの世界: 柱が並ぶ。上下 = 依存の向き、柱の高さ = 行数
    let files: Vec<&analyze::Node> = a.nodes.iter().filter(|n| n.kind == analyze::Kind::File).collect();
    let cx = files.iter().map(|n| n.x).sum::<f32>() / files.len().max(1) as f32;
    let cz = files.iter().map(|n| n.z).sum::<f32>() / files.len().max(1) as f32;
    let height = |lines: usize| 6.0 + lines as f32 * 1.6;
    let layer_gap = files.iter().map(|n| height(n.size)).fold(10.0, f32::max) + 22.0;
    const SPREAD: f32 = 24.0;

    let mut w = World::new(Law::Pillars);
    let mut index = vec![0usize; a.nodes.len()];
    for (ai, an) in a.nodes.iter().enumerate() {
        let at = Vec3::new((an.x - cx) * SPREAD, an.layer as f32 * layer_gap, (an.z - cz) * SPREAD);
        index[ai] = w.items.len();
        match an.kind {
            analyze::Kind::File => {
                let i = match parse::parse_file(&root.join(&an.name), an.name.clone()) {
                    Ok(pn) => t.add(&pn, Some(world)),
                    Err(e) => {
                        eprintln!("{e}");
                        t.push(an.name.clone(), Kind::File, an.size, Some(world))
                    }
                };
                t.item(&mut w, world, i, at, Vec3::new(11.0, height(an.size), 11.0));
            }
            analyze::Kind::Package => {
                let i = t.push(an.name.clone(), Kind::Package, an.size, Some(world));
                let f = (an.size as f32 + 1.0).ln(); // 大きさ = 中のファイル数(対数)
                t.item(&mut w, world, i, at, Vec3::new(6.0 + f * 2.4, 4.0 + f * 8.0, 6.0 + f * 2.4));
            }
        }
    }
    for (ai, an) in a.nodes.iter().enumerate() {
        for &d in &an.deps {
            w.links.push((index[ai], index[d], an.kind == analyze::Kind::File));
        }
    }
    let top = files.iter().map(|n| n.layer).max().unwrap_or(0) as f32 * layer_gap;
    let reach = files.iter().map(|n| ((n.x - cx).powi(2) + (n.z - cz).powi(2)).sqrt()).fold(1.0, f32::max) * SPREAD;
    // 一番上の層の、一番高い柱のそばに立つ。そこから、頼っている先が下へ続くのが見える
    let near = w.items.iter().filter(|it| it.pos.y >= top - 0.1).max_by(|a, b| a.dims.y.total_cmp(&b.dims.y)).map_or(Vec3::ZERO, |it| it.pos);
    let out = Vec3::new(near.x, 0.0, near.z).normalize_or(Vec3::Z);
    let _ = reach;
    w.stand(near + out * 30.0 + Vec3::new(-out.z, 0.0, out.x) * 16.0 + Vec3::Y * 5.0, Vec3::new(0.0, top * 0.5, 0.0));
    w.radius *= 1.5;
    t.n[world].world = Some(w);
    t
}
