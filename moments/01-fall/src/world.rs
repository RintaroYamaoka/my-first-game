//! 本物のプロジェクトから、「世界の集まり」を組み立てる。
//!   中身を持つものは、それぞれが一つの世界を持つ。世界は、外から見た形の中には入っていない。
//!   外から見た形は入口の姿で、潜った先は、その世界だけの広さと法則を持つ。
//! 法則(世界の種類ごとに、並べ方が違う):
//!   Pillars … プロジェクト。柱が並ぶ。上下 = 依存の向き
//!   Shaft   … ファイル。中が空洞の塔(縦坑)。書かれたものは壁から突き出て、らせんに下へ続く。持っている状態は、中心の軸に浮かぶ
//!   Road    … 関数。実行の順に前へ進む道。分岐で道が分かれ、return は道をふさぐ壁
//!   Ring    … ループ。道が輪になって、元の場所に戻る

use crate::analyze;
use crate::parse;
pub use crate::parse::How;
use bevy::math::{Quat, Vec3};
use std::f32::consts::{FRAC_PI_2, TAU};
use std::path::Path;

const ROAD_W: f32 = 16.0;
const LANE_GAP: f32 = 9.0;
const GAP: f32 = 7.0;
const FORK: f32 = 16.0;
const FLOW_Y: f32 = 0.38;
const GATE_H: f32 = 11.0;
const POST: f32 = 1.3;
/// 目の高さ(足の裏から)
pub const EYE: f32 = 2.6;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Law {
    Pillars,
    Shaft,
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

/// 形の役割(縦坑の世界で、見た目と、起動のときの動きを分ける)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    /// ふつうの形(関数、import、型など)
    Plain,
    /// 関数の外に書かれた処理。読み込まれたときに一度だけ走る「点火点」
    Spark,
    /// 変わらない値。固い結晶
    Fixed,
    /// 起動のときに、計算で埋まる値。最初は空の枠で、呼んだ先から光が戻ると、中身が入る
    Filled,
    /// あとから書き換えられる値。ゆっくり脈を打つ
    Live,
}

/// 世界の中に置かれた形ひとつ
pub struct Item {
    pub role: Role,
    pub node: usize,
    pub pos: Vec3,
    pub dims: Vec3,
    pub yaw: f32,
    /// 面を持つ形か(型は、線だけの枠)
    pub solid: bool,
    /// 中の区切りを、外から見える帯として出す(上からの割合)
    pub bands: Vec<f32>,
    /// 形を作る箱(自分の中での位置、大きさ)。空なら、全体が一つの箱
    pub parts: Vec<(Vec3, Vec3)>,
}

impl Item {
    /// 形を作る箱を、世界の中の位置で返す(中心、向き、大きさ)
    pub fn boxes(&self) -> Vec<(Vec3, Quat, Vec3)> {
        let rot = Quat::from_rotation_y(self.yaw);
        if self.parts.is_empty() {
            return vec![(self.pos, rot, self.dims)];
        }
        self.parts.iter().map(|&(off, dims)| (self.pos + rot * off, rot, dims)).collect()
    }
}

pub struct World {
    pub law: Law,
    pub items: Vec<Item>,
    /// 実行の流れの線
    pub paths: Vec<Vec<Vec3>>,
    /// 流れの線のうち、道がないもの(何もしない側)
    pub thin: Vec<Vec<Vec3>>,
    /// 形ではない面: 道、壁(位置、大きさ、向き)
    pub decor: Vec<(Vec3, Vec3, f32)>,
    /// 足場: 立てるが、面は無く、線の格子だけ(下の世界が透けて見える)
    pub footholds: Vec<(Vec3, Vec3, f32)>,
    /// 依存の線(頼っている側、頼られている側、依存の種類)。Item の番号
    pub links: Vec<(usize, usize, How)>,
    /// 潜ってきたときに立つ場所と向き
    pub spawn: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    /// 世界の広さ(霞の濃さを決める)
    pub radius: f32,
    /// 依存が、この世界の外へ続いている形(空への光が立つ)。Item の番号と、行き先がプログラムの外か(外なら、光が明滅する)
    pub sky: Vec<(usize, bool)>,
}

impl World {
    fn new(law: Law) -> Self {
        World { law, items: vec![], paths: vec![], thin: vec![], decor: vec![], footholds: vec![], links: vec![], spawn: Vec3::ZERO, yaw: 0.0, pitch: 0.0, radius: 60.0, sky: vec![] }
    }

    /// 足場を置く。立てる板だが、面は無く、線の格子で見せる
    fn foothold(&mut self, pos: Vec3, dims: Vec3, yaw: f32) {
        self.footholds.push((pos, dims, yaw));
        let rot = Quat::from_rotation_y(yaw);
        let (half, top) = (dims * 0.5, dims.y * 0.5);
        const CELLS: usize = 6;
        for n in 0..=CELLS {
            let u = n as f32 / CELLS as f32 * 2.0 - 1.0;
            self.thin.push(vec![pos + rot * Vec3::new(half.x * u, top, -half.z), pos + rot * Vec3::new(half.x * u, top, half.z)]);
            self.thin.push(vec![pos + rot * Vec3::new(-half.x, top, half.z * u), pos + rot * Vec3::new(half.x, top, half.z * u)]);
        }
    }

    fn stand(&mut self, spawn: Vec3, target: Vec3) {
        self.spawn = spawn;
        (self.yaw, self.pitch) = look(spawn, target);
        let far = self.items.iter().map(|it| it.pos.distance(spawn) + it.dims.max_element() * 0.5).fold(0.0, f32::max);
        self.radius = far.max(60.0);
    }
}

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
    /// 宣言している名前、使っている名前とその使い方、プログラムの外に触っているか(構文解析から)
    pub declares: Vec<String>,
    pub uses: Vec<(String, How)>,
    pub outside: bool,
    /// 自分の世界の外にある名前(これを使っている形の依存は、世界の外へ続く)
    pub outer: Vec<String>,
}

pub struct Tree {
    pub n: Vec<N>,
    /// プログラムの外に触るモジュール(ファイル、ネットワークなど)から import した名前
    outside_names: Vec<String>,
}

fn line(x: f32, z0: f32, z1: f32) -> Vec<Vec3> {
    vec![Vec3::new(x, FLOW_Y, z0), Vec3::new(x, FLOW_Y, z1)]
}

impl Tree {
    fn push(&mut self, name: String, kind: Kind, lines: usize, parent: Option<usize>) -> usize {
        self.n.push(N { name, kind, lines, kids: vec![], world: None, owner: None, declares: vec![], uses: vec![], outside: false, outer: vec![] });
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
        (self.n[i].declares, self.n[i].uses, self.n[i].outside) = (pn.declares.clone(), pn.uses.clone(), pn.outside);
        if kind == Kind::Import && pn.outside {
            self.outside_names.extend(pn.declares.iter().cloned());
        }
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
            // 中の世界から見た「外の名前」: この世界の外の名前と、この世界で宣言された名前(自分の中の物は除く)。
            // ファイルの外の名前は、import を通ってしか入ってこないので、ファイルの世界には「外の名前」が無い
            if self.n[k].kind != Kind::File {
                let mut outer = self.n[owner].outer.clone();
                self.names(owner, k, &mut outer);
                self.n[k].outer = outer;
            }
            self.make_world(k);
        }
        self.n[k].owner = Some((owner, w.items.len()));
        let bands = if enter { self.bands(k) } else { vec![] };
        w.items.push(Item { role: Role::Plain, node: k, pos, dims, yaw: 0.0, solid: self.n[k].kind != Kind::Type, bands, parts: vec![] });
    }

    fn make_world(&mut self, i: usize) {
        let w = match self.n[i].kind {
            Kind::File => self.shaft(i),
            Kind::Loop => self.ring(i),
            _ => self.road(i),
        };
        self.n[i].world = Some(w);
    }

    /// 節 i の世界で宣言されている名前を集める(skip の下は除く)。ほかの関数の中で宣言された名前は、外からは見えないので、入れない
    fn names(&self, i: usize, skip: usize, out: &mut Vec<String>) {
        for &c in &self.n[i].kids {
            if c != skip {
                out.extend(self.n[c].declares.iter().cloned());
                if self.n[c].kind != Kind::Fn {
                    self.names(c, skip, out);
                }
            }
        }
    }

    /// 世界の中の形を、依存の線で結ぶ: 形 a が、形 b の宣言した名前を使っていれば、a は b に頼っている。
    /// 使っている名前が、この世界の外の物(i の outer)なら、依存は世界の外へ続く。import と、プログラムの外に触る形も、同じ
    fn connect(&self, w: &mut World, i: usize) {
        for (a, ia) in w.items.iter().enumerate() {
            let na = &self.n[ia.node];
            for (b, ib) in w.items.iter().enumerate() {
                let how = na.uses.iter().filter(|u| self.n[ib.node].declares.contains(&u.0)).map(|u| u.1).max();
                if let (true, Some(how)) = (a != b, how) {
                    w.links.push((a, b, how));
                }
            }
            let outer = |name: &String| self.n[i].outer.contains(name);
            let leaves = na.kind == Kind::Import || na.uses.iter().any(|u| outer(&u.0));
            // プログラムの外: 自分で外に触っているか、外に触るモジュールから import した名前を、この世界の外から使っている
            let outside = na.outside || na.uses.iter().any(|u| outer(&u.0) && self.outside_names.contains(&u.0));
            if leaves || outside {
                w.sky.push((a, outside));
            }
        }
    }

    /// ファイルの世界: 中が空洞の塔(縦坑)。書かれたものは内側の壁から突き出て、書かれた順に、らせんを描いて下へ続く。
    /// ファイルが持っている状態(const や let)は、壁ではなく、中心の軸に浮かぶ。使う・呼ぶの関係は、空洞を横切る依存の線
    fn shaft(&mut self, i: usize) -> World {
        const TURN: f32 = 0.92; // 1 つ進むごとに、らせんを回る角度
        let kids = self.n[i].kids.clone();
        let r = (kids.len() as f32 * 4.0).clamp(52.0, 110.0);
        let mut w = World::new(Law::Shaft);
        let mut top = 0.0f32;
        let mut spiral = vec![];
        for (n, &k) in kids.iter().enumerate() {
            let (kind, lines) = (self.n[k].kind, self.n[k].lines as f32);
            let a = n as f32 * TURN;
            let out = Vec3::new(a.sin(), 0.0, a.cos());
            let label = self.n[k].name.clone();
            let state = matches!(kind, Kind::Stmt | Kind::Call) && !self.n[k].declares.is_empty();
            // 関数の外に書かれた処理(変数の定義でも、関数でも、import でも、型でもないもの)
            let spark = !state && !matches!(kind, Kind::Import | Kind::Fn | Kind::Type);
            let role = if spark {
                Role::Spark
            } else if !state {
                Role::Plain
            } else {
                // 変数の性質を、書き方から見分ける(粗い見方): 呼び出しがあれば「起動で埋まる」、let や、中身を足せる入れ物なら「書き換えられる」
                let value = label.split_once('=').map_or("", |(_, v)| v).trim();
                if value.contains('(') {
                    Role::Filled
                } else if !label.contains("const ") || value.starts_with('[') || value.starts_with('{') {
                    Role::Live
                } else {
                    Role::Fixed
                }
            };
            // 大きさ(幅、高さ、壁からの出っ張り)は、種類で変える
            let dims = match kind {
                Kind::Import => Vec3::new(5.0, 3.0, 13.0),
                Kind::Type => Vec3::new(15.0, 4.0 + lines * 1.2, 7.0),
                _ if state => Vec3::splat(7.0 + lines.min(6.0)),
                _ if spark => Vec3::splat(4.5),
                _ if self.n[k].kids.is_empty() => Vec3::new(9.0, 5.0, 9.0),
                _ => Vec3::new(24.0, 8.0 + lines * 1.5, 20.0),
            };
            let y = top - dims.y * 0.5;
            // 状態は中心の軸、点火点はらせんの線の上、ほかは壁から突き出る
            let pos = if state { Vec3::new(0.0, y, 0.0) } else if spark { out * (r - 4.0) + Vec3::Y * y } else { out * (r - dims.z * 0.5) + Vec3::Y * y };
            self.item(&mut w, i, k, pos, dims);
            let it = w.items.last_mut().unwrap();
            it.yaw = if state { a + 0.6 } else { a };
            it.role = role;
            if spark {
                it.solid = false; // 点火点は、面を持たない(走った跡の、線だけの印)
                it.bands.clear();
            }
            // らせんの線: 壁に沿って、次の形へ降りていく
            for s in 0..=4 {
                let b = a + TURN * s as f32 / 4.0;
                spiral.push(Vec3::new(b.sin(), 0.0, b.cos()) * (r - 1.5) + Vec3::Y * (top - (dims.y * 0.45 + 9.0) * s as f32 / 4.0));
            }
            top -= dims.y * 0.45 + 9.0;
        }
        w.paths.push(spiral);
        self.connect(&mut w, i);
        // 壁: 縦の板を、すき間を空けて並べる(すき間から、外の霞が見える)。板の端と、一定の深さごとの輪を、線で引く
        let depth = -top + 60.0;
        const SLATS: usize = 18;
        for n in 0..SLATS {
            let a = (n as f32 + 0.5) / SLATS as f32 * TAU;
            let half = TAU / SLATS as f32 * 0.31;
            w.decor.push((Vec3::new(a.sin(), 0.0, a.cos()) * (r + 2.0) + Vec3::Y * (34.0 - depth * 0.5), Vec3::new((r + 2.0) * half * 2.0, depth + 68.0, 2.5), a));
            for b in [a - half, a + half] {
                let p = Vec3::new(b.sin(), 0.0, b.cos()) * (r + 0.6);
                w.thin.push(vec![p + Vec3::Y * 68.0, p - Vec3::Y * depth]);
            }
        }
        let mut y = 68.0;
        while y > -depth - 1.0 {
            w.thin.push((0..=48).map(|n| { let a = n as f32 / 48.0 * TAU; Vec3::new(a.sin() * (r + 0.6), y, a.cos() * (r + 0.6)) }).collect());
            y -= 34.0;
        }
        // 穴の口の縁に立ち、空洞をのぞき込む。立つ場所には、壁から張り出した足場がある
        let (sx, sz) = (-r * 0.25, r * 0.8);
        let sa = sx.atan2(sz);
        w.foothold(Vec3::new(sa.sin(), 0.0, sa.cos()) * (r * 0.84) + Vec3::Y * (20.0 - EYE - 0.3), Vec3::new(22.0, 0.6, r * 0.34), sa);
        w.stand(Vec3::new(-r * 0.25, 20.0, r * 0.8), Vec3::new(r * 0.1, -depth * 0.45, -r * 0.35));
        w.radius = (depth * 1.1 + r * 2.0).max(220.0);
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
        self.connect(&mut w, i);
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
        self.connect(&mut w, i);
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
    let mut t = Tree { n: vec![], outside_names: vec![] };
    let name = root.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "world".into());
    let world = t.push(format!("{name}/"), Kind::World, 0, None);

    // プロジェクトの世界: 柱が並ぶ。上下 = 依存の向き(頼っている先ほど下)、柱の高さ = 行数
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
            w.links.push((index[ai], index[d], How::Import));
        }
    }
    let top = files.iter().map(|n| n.layer).max().unwrap_or(0) as f32 * layer_gap;
    // 一番上の層の、一番高い柱のそばに立つ。そこから、頼っている先が下へ続くのが見える
    let near = w.items.iter().filter(|it| it.pos.y >= top - 0.1).max_by(|a, b| a.dims.y.total_cmp(&b.dims.y)).map_or(Vec3::ZERO, |it| it.pos);
    let out = Vec3::new(near.x, 0.0, near.z).normalize_or(Vec3::Z);
    w.stand(near + out * 30.0 + Vec3::new(-out.z, 0.0, out.x) * 16.0 + Vec3::Y * 5.0, Vec3::new(0.0, top * 0.5, 0.0));
    w.radius *= 1.5;
    // 立つ場所の足場(柱は宙に浮いているので、何もないと立てない)
    w.foothold(w.spawn - Vec3::Y * (EYE + 0.3), Vec3::new(18.0, 0.6, 18.0), 0.0);

    // node_modules/ の世界: 依存パッケージが、巨大な柱の街になる。柱の高さと太さ = 中のファイル数。依存の線 = パッケージ同士の依存
    let packages: Vec<(usize, &analyze::Node)> = a.nodes.iter().enumerate().filter(|(_, n)| n.kind == analyze::Kind::Package).collect();
    if !packages.is_empty() {
        const BLOCK: f32 = 78.0; // 柱と柱の間(通り)の広さを含む、1 区画の幅
        let city = t.push("node_modules/".into(), Kind::World, packages.len(), Some(world));
        let mut c = World::new(Law::Pillars);
        let side = (packages.len() as f32).sqrt().ceil() as usize;
        let mut inside = vec![None; a.nodes.len()];
        for (k, &(ai, an)) in packages.iter().enumerate() {
            let f = (an.size as f32 + 1.0).ln();
            let dims = Vec3::new(24.0 + f * 5.0, 30.0 + f * f * 13.0, 24.0 + f * 5.0);
            let cell = |g: usize| (g as f32 - (side - 1) as f32 * 0.5) * BLOCK;
            let i = t.push(an.name.clone(), Kind::Package, an.size, Some(city));
            inside[ai] = Some(c.items.len());
            t.item(&mut c, city, i, Vec3::new(cell(k % side), dims.y * 0.5, cell(k / side)), dims);
        }
        for &(ai, an) in &packages {
            for &d in &an.deps {
                if let (Some(from), Some(to)) = (inside[ai], inside[d]) {
                    c.links.push((from, to, How::Import));
                }
            }
        }
        // 街の端の通りに、地面の高さで立ち、街の中心を見る
        let edge = (side as f32 * 0.5 + 0.3) * BLOCK;
        c.stand(Vec3::new(BLOCK * 0.5, EYE, edge), Vec3::new(0.0, 60.0, 0.0));
        t.n[city].world = Some(c);
        // 入口: プロジェクトの世界の、立つ場所の近くに置く平たい箱
        let spawn = w.spawn;
        t.n[city].owner = Some((world, w.items.len()));
        w.items.push(Item { role: Role::Plain, node: city, pos: spawn + Vec3::new(-out.z, 0.0, out.x) * 34.0 + out * 6.0 - Vec3::Y * 4.0, dims: Vec3::new(26.0, 7.0, 26.0), yaw: 0.0, solid: true, bands: vec![], parts: vec![] });
    }
    t.n[world].world = Some(w);
    t
}
