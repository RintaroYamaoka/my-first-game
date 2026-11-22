//! 見せ場 1(段階 2): 下の層へ落ちる瞬間。世界の作りは、段階 1 の実験 2c から引き継いだ。
//!   cargo run -p fall -- <プロジェクトのフォルダ> [曲のパートのフォルダ]
//! 操作:
//!   マウス 見回す / W A S D 歩く / Shift 走る / F 跳ぶ / Space ブリンク(決まった距離を跳ぶ。移動キーを押していれば、その向きへ)
//!   左クリック・E 照準の先に潜る / 右クリック・Q 上の世界へ戻る / Esc マウスを放す
//! 確かめるための指定(環境変数)は、`Check` にまとめてある

mod analyze;
mod parse;
mod sound;
mod world;

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::post_process::bloom::{Bloom, BloomCompositeMode, BloomPrefilter};
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::{CursorGrabMode, CursorOptions};
use world::{How, Law, Role, Tree, EYE};

const ACCENT: Vec3 = Vec3::new(0.898, 0.263, 0.165);
const FOG_CAP: f32 = 0.88; // 霞みきらない。遠くの巨大な構造が、薄い影として残る
/// 歩く速さ、走る速さ。速く遠くへ動く手段は、ブリンクだけ
const WALK: f32 = 5.5;
const RUN: f32 = 9.5;
/// 確かめるときの、浮いて動く速さ(BODY=ghost)
const FLY: f32 = 40.0;
const BLINK: f32 = 80.0;
const BLINK_SIDE: f32 = 40.0;
/// ブリンクが、形の手前で止まる距離
const BLINK_STOP: f32 = 3.0;
/// ブリンクが形の縁に当たったとき、天辺までがこの高さ以内なら、上に乗る。乗るときに、縁から内へ入る距離
const LEDGE: f32 = 6.0;
const LEDGE_IN: f32 = 1.3;
/// ブリンクの時間の割り振り。面がほどける → 線だけの世界を渡る → 着いた先で面が戻る
const UNMAKE_SEC: f32 = 0.05;
const CROSS_SEC: f32 = 0.11;
const REMAKE_SEC: f32 = 0.24;
/// 縦坑の世界の起動: 着いてから待つ秒、全部が組み上がるまでの秒
const BOOT_WAIT: f32 = 0.9;
const BOOT_SEC: f32 = 3.4;
/// 起動で埋まる値に、光が行って戻るまでの秒
const FILL_SEC: f32 = 0.7;
/// 影響の範囲が、1 段たどるのにかける秒
const HOP_SEC: f32 = 0.16;
// 依存の線は、光として描く(2026-11-22 本人が決めた)
/// 光の明るさ(1 を超えた分が、にじむ)。影響の範囲と循環は赤い光、頼っている先と空への光は白い光
const LIGHT: f32 = 5.0;
const WHITE: Vec3 = Vec3::new(1.0, 0.97, 0.9);
/// 依存の線が出ている間、世界を暗くして、光を浮かび上がらせる。暗くした後の明るさと、変わる速さ
const DIM: f32 = 0.42;
const DIM_SPEED: f32 = 5.0;
/// 弧の高さ(両端の距離に対する割合)
const ARC_LIFT: f32 = 0.22;
/// 空への光: 伸びる高さと、伸びきるまでの秒
const SKY_H: f32 = 700.0;
const SKY_SEC: f32 = 0.35;
const AMBIENT: f32 = 730.0;
const SUN: f32 = 1900.0;
/// 残像の枚数
const GHOSTS: usize = 6;
/// 着いた瞬間の乱れ。線が粗い目盛りから 3 段で正しい位置に収まる時間と、化けた名前が頭から解ける時間
const SETTLE_SEC: f32 = 0.1;
const RESOLVE_SEC: f32 = 0.7;
/// 目盛りの粗さ(見える範囲に対する割合)。粗い → 細かい
const SETTLE_GRID: [f32; 3] = [0.03, 0.015, 0.006];
/// 化けた名前に使う文字
const NOISE: &[u8] = b"01<>[]{}#$%&*+=_:;?/\\|^~";
const FOV: f32 = 62.0;
const SENS: f32 = 0.0022;
const OUT_SEC: f32 = 0.5;
const IN_SEC: f32 = 1.1;
// 体の決まり: 重力があり、形は固い。動き回るのは、ブリンクが基本(2026-11-22 本人が決めた)
/// 足もとの球の半径、頭の球の半径(目の高さは world::EYE)
const FOOT_R: f32 = 1.2;
const HEAD_R: f32 = 0.7;
/// 重力。どの世界でも同じ(2026-11-22 本人が決めた)
const GRAVITY: f32 = 21.0;
const JUMP: f32 = 13.0;
const FALL_MAX: f32 = 70.0;
/// 一歩の長さ(足音の間隔)と、歩くときの視点の揺れ(ごく小さく。大きいと酔う)
const STRIDE: f32 = 2.3;
const BOB: f32 = 0.045;
/// 着地: これより速く落ちてきたら、音が鳴り、視点が沈む。沈む深さ(落ちる速さあたり)、その上限、戻る速さ
const LAND_MIN: f32 = 6.0;
const DIP: f32 = 0.035;
const DIP_MAX: f32 = 1.0;
const DIP_BACK: f32 = 9.0;

/// 確かめるための指定(環境変数)。起動のときに一度だけ読む
#[derive(Resource)]
struct Check {
    /// GO=a,b: 名前の一部で、順に潜る(これだけ付けると、その世界から始められる)
    go: Vec<String>,
    /// SHOT=<png>: 画面を 1 枚撮って終わる。WAIT=<秒>: 撮るまでに待つ秒
    shot: Option<String>,
    wait: f32,
    /// TURN=yaw,pitch: 撮る前に、今の向きから、その分だけ向きを変える
    turn: Option<Vec<f32>>,
    /// AIM=<名前の一部>: 撮る前に、その形に照準を合わせる
    aim: Option<String>,
    /// POSE=x,y,z,yaw,pitch: 撮る前の視点
    pose: Option<Vec<f32>>,
    /// HOLD=W: 前へ歩くキーを押したままにする
    hold: bool,
    /// TAP=Space: 視点を決めた直後に、一度だけブリンクのキーを押す
    tap: bool,
    /// FREEZE=<秒>: ブリンクの途中で時間を止める(途中の絵を撮るため)
    freeze: Option<f32>,
    /// LINKS=1: 起動のときに、全部の世界の依存の線を文字で出す
    links: bool,
    /// BODY=ghost: 体を持たず、浮いて、何でも通り抜ける
    ghost: bool,
}

impl Check {
    fn read() -> Check {
        let var = |k: &str| std::env::var(k).ok();
        let nums = |k: &str, n: usize| var(k).map(|v| v.split(',').filter_map(|s| s.trim().parse().ok()).collect::<Vec<f32>>()).filter(|v| v.len() == n);
        Check {
            go: var("GO").unwrap_or_default().split(',').filter(|s| !s.is_empty()).map(str::to_string).collect(),
            shot: var("SHOT"),
            wait: var("WAIT").and_then(|v| v.parse().ok()).unwrap_or(1.0),
            turn: nums("TURN", 2),
            aim: var("AIM"),
            pose: nums("POSE", 5),
            hold: var("HOLD").is_some_and(|k| k == "W"),
            tap: var("TAP").is_some_and(|k| k == "Space"),
            freeze: var("FREEZE").and_then(|v| v.parse().ok()),
            links: var("LINKS").is_some(),
            ghost: var("BODY").is_some_and(|v| v == "ghost"),
        }
    }
}

/// ぶつかる箱(位置、向き、大きさの半分)
struct Solid {
    pos: Vec3,
    rot: Quat,
    half: Vec3,
}

/// 今ぶつかる箱: 道・壁と、足場と、面を持つ形。縦坑の起動でまだ面が現れていない形には、ぶつからない(since = 着いてからの秒)
fn solids(w: &world::World, since: f32) -> Vec<Solid> {
    let mut out: Vec<Solid> = w.decor.iter().chain(&w.footholds).map(|&(pos, dims, yaw)| Solid { pos, rot: Quat::from_rotation_y(yaw), half: dims * 0.5 }).collect();
    for (j, it) in w.items.iter().enumerate() {
        if it.solid && solid_at(w, j) <= since {
            out.extend(it.boxes().into_iter().map(|(pos, rot, dims)| Solid { pos, rot, half: dims * 0.5 }));
        }
    }
    out
}

/// 地面の高さ(いちばん低い形の下の面)。その下へは落ちない
fn ground_of(w: &world::World) -> f32 {
    let items = w.items.iter().filter(|it| it.solid).flat_map(|it| it.boxes()).map(|(pos, _, dims)| pos.y - dims.y * 0.5);
    w.decor.iter().chain(&w.footholds).map(|&(pos, dims, _)| pos.y - dims.y * 0.5).chain(items).fold(0.0, f32::min)
}

/// 箱の表面から、点 p への向きと距離(箱の中にいるときは、距離が負)
fn near(s: &Solid, p: Vec3) -> (Vec3, f32) {
    let l = s.rot.inverse() * (p - s.pos);
    let d = l - l.clamp(-s.half, s.half);
    if d.length_squared() > 1e-8 {
        let len = d.length();
        return (s.rot * (d / len), len);
    }
    let gap = s.half - l.abs();
    let axis = if gap.x <= gap.y && gap.x <= gap.z { Vec3::X * l.x.signum() } else if gap.y <= gap.z { Vec3::Y * l.y.signum() } else { Vec3::Z * l.z.signum() };
    (s.rot * axis, -gap.min_element())
}

/// 球を、形と地面の外へ押し出す。押された向きを hits に足す
fn push_out(solids: &[Solid], ground: f32, mut c: Vec3, r: f32, hits: &mut Vec<Vec3>) -> Vec3 {
    for _ in 0..3 {
        if c.y - ground < r {
            c.y = ground + r;
            hits.push(Vec3::Y);
        }
        for s in solids {
            let (n, d) = near(s, c);
            if d < r {
                c += n * (r - d);
                hits.push(n);
            }
        }
    }
    c
}

/// 視線が、最初に当たるまでの距離と、当たった箱(地面なら None)
fn ray(solids: &[Solid], ground: f32, from: Vec3, dir: Vec3) -> (f32, Option<usize>) {
    let mut best = (if dir.y < -1e-4 && from.y > ground { (ground - from.y) / dir.y } else { f32::MAX }, None);
    for (i, s) in solids.iter().enumerate() {
        let inv = s.rot.inverse();
        let (o, d) = (inv * (from - s.pos), inv * dir);
        let (t1, t2) = ((-s.half - o) / d, (s.half - o) / d);
        let (a, b) = (t1.min(t2).max_element(), t1.max(t2).min_element());
        if a <= b && a > 0.0 && a < best.0 {
            best = (a, Some(i));
        }
    }
    best
}

/// 体を動かして、形にぶつかった分を押し戻す。速いときに突き抜けないよう、細かく刻む
fn step_body(nav: &mut Nav, solids: &[Solid], ground: f32, mv: Vec3) {
    let steps = ((mv.length() / 0.5).ceil() as usize).clamp(1, 60);
    let mut hits = vec![];
    let off = Vec3::Y * (EYE - FOOT_R);
    for _ in 0..steps {
        nav.pos += mv / steps as f32;
        nav.pos = push_out(solids, ground, nav.pos - off, FOOT_R, &mut hits) + off;
        nav.pos = push_out(solids, ground, nav.pos, HEAD_R, &mut hits);
    }
    nav.grounded = hits.iter().any(|n| n.y > 0.6);
    if (nav.grounded && nav.fall < 0.0) || (nav.fall > 0.0 && hits.iter().any(|n| n.y < -0.6)) {
        nav.fall = 0.0;
    }
}

/// 歩く・跳ぶ・落ちる。wish = 歩きたい向き(水平)
fn walk(nav: &mut Nav, solids: &[Solid], ground: f32, wish: Vec3, speed: f32, jump: bool, dt: f32) {
    if nav.grounded && jump {
        nav.fall = JUMP;
    }
    nav.fall = (nav.fall - GRAVITY * dt).max(-FALL_MAX);
    let (before, was_grounded, falling) = (nav.pos, nav.grounded, -nav.fall);
    step_body(nav, solids, ground, wish * speed * dt + Vec3::Y * nav.fall * dt);
    if nav.grounded {
        if !was_grounded && falling > LAND_MIN {
            nav.landed = Some(falling);
            nav.dip = (falling * DIP).min(DIP_MAX);
        }
        // 歩いた距離が一歩ぶん進むたびに、足音
        let steps = (nav.stride / STRIDE) as u32;
        nav.stride += (nav.pos - before).with_y(0.0).length();
        nav.stepped = (nav.stride / STRIDE) as u32 != steps;
    }
    nav.dip *= (-DIP_BACK * dt).exp();
}

/// ブリンクの着く先。形に当たるなら、その手前で止まる。当たった所が形の天辺か、天辺のすぐ下の縁なら、その上に乗る
fn blink_to(solids: &[Solid], ground: f32, from: Vec3, dir: Vec3, len: f32) -> Vec3 {
    let (hit, which) = ray(solids, ground, from, dir);
    if hit >= len {
        return from + dir * len;
    }
    let p = from + dir * hit;
    if let Some(s) = which.map(|i| &solids[i]) {
        let top = s.pos.y + s.half.y;
        if top - p.y <= LEDGE {
            return Vec3::new(p.x, top + EYE + 0.05, p.z) + Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero() * LEDGE_IN;
        }
    }
    from + dir * (hit - BLINK_STOP).max(0.0)
}

/// 世界の法則ごとの、空気(地の色、線の色、面の色)
struct Air {
    bg: Vec3,
    ink: Vec3,
    face: Vec3,
}

fn air_of(law: Law) -> Air {
    match law {
        Law::Pillars => Air { bg: Vec3::new(0.914, 0.914, 0.898), ink: Vec3::new(0.07, 0.07, 0.07), face: Vec3::ONE },
        Law::Shaft => Air { bg: Vec3::new(0.76, 0.82, 0.86), ink: Vec3::new(0.05, 0.10, 0.17), face: Vec3::new(0.95, 0.98, 1.0) },
        Law::Road => Air { bg: Vec3::new(0.93, 0.86, 0.76), ink: Vec3::new(0.17, 0.09, 0.05), face: Vec3::new(1.0, 0.97, 0.91) },
        Law::Ring => Air { bg: Vec3::new(0.055, 0.065, 0.085), ink: Vec3::new(0.86, 0.89, 0.92), face: Vec3::new(0.17, 0.19, 0.23) },
    }
}

fn rgb(v: Vec3, a: f32) -> Color {
    Color::srgba(v.x, v.y, v.z, a)
}

/// 光の色: 明るさを LIGHT 倍にする(1 を超えた分が、にじむ)
fn glow(v: Vec3, a: f32) -> Color {
    let c = Color::srgb(v.x, v.y, v.z).to_linear();
    Color::linear_rgba(c.red * LIGHT, c.green * LIGHT, c.blue * LIGHT, a)
}

#[derive(Resource)]
struct Scape(Tree);

/// 世界を渡る動き: 前半は今の世界が霞に消え、後半は次の世界へ落ちてくる(戻るときは浮かび上がる)
struct Tr {
    t: f32,
    switched: bool,
    from: Vec3,
    to: Vec3,
    next: usize,
    arrive: Vec3,
    drop: Vec3,
    yaw: f32,
    pitch: f32,
}

#[derive(Clone, Copy)]
struct Blink {
    from: Vec3,
    to: Vec3,
    t: f32,
}

impl Blink {
    /// まだ渡っている途中か(この間は、歩けない)
    fn crossing(&self) -> bool {
        self.t < UNMAKE_SEC + CROSS_SEC
    }
}

#[derive(Resource)]
struct Nav {
    world: usize,
    pos: Vec3,
    yaw: f32,
    pitch: f32,
    /// 照準の先にあるもの(Item の番号)
    aim: Option<usize>,
    tr: Option<Tr>,
    veil: f32,
    rebuild: bool,
    changed: bool,
    /// このコマでブリンクした(音を鳴らす合図)
    blinked: bool,
    /// ブリンクの途中: 出発点、到着点、押してからの秒
    blink: Option<Blink>,
    /// 今の世界に着いた時刻(縦坑の世界が、着いてから順に組み上がるため)
    born: f32,
    /// 上下の速さ(上が正)
    fall: f32,
    /// 地面か、形の上に立っているか
    grounded: bool,
    /// 歩いた距離(足音と、視点の揺れのため)
    stride: f32,
    /// 着地で視点が沈んでいる深さ
    dip: f32,
    /// 照準を合わせた形に、依存の線があるか(ある間、世界が暗くなる)
    linked: bool,
    /// このコマで一歩進んだ / 着地した(落ちてきた速さ)。音を鳴らす合図
    stepped: bool,
    landed: Option<f32>,
}

#[derive(Resource, Default)]
struct Shown {
    slabs: Vec<Entity>,
    labels: Vec<Entity>,
}

#[derive(Component)]
struct Label;
/// 面が、何番目の形のものか(起動の順に現れるため)
#[derive(Component)]
struct Order(usize);
#[derive(Component)]
struct Hud;
#[derive(Component)]
struct Veil;
#[derive(Component)]
struct Cross;

#[derive(Default, Reflect, GizmoConfigGroup)]
struct AccentLines;
/// 空への光(太い)
#[derive(Default, Reflect, GizmoConfigGroup)]
struct SkyLines;

fn main() {
    let root = std::env::args().nth(1).unwrap_or_else(|| "sample".into());
    let tree = world::build(std::path::Path::new(&root));
    println!("節 {} 個、世界 {} 個", tree.n.len(), tree.n.iter().filter(|n| n.world.is_some()).count());

    // 2 つ目の引数: 曲のパートが入ったフォルダ(無ければ、潜る・戻る音だけ)
    let music = std::env::args().nth(2).map(std::path::PathBuf::from);
    let audio = sound::Audio::new(music.as_deref(), OUT_SEC, IN_SEC).expect("音の準備に失敗");
    for n in &tree.n {
        if let Some(w) = &n.world {
            let c = cycles(w).iter().filter(|&&r| r).count();
            if c > 0 {
                println!("循環: {} に、輪になった線が {} 本", n.name, c);
            }
        }
    }
    println!("曲: {}", if audio.has_music() { "あり" } else { "なし" });
    let check = Check::read();
    if check.links {
        for n in &tree.n {
            let Some(w) = &n.world else { continue };
            let name = |j: usize| tree.n[w.items[j].node].name.as_str();
            println!("世界 {}", tree.chain(w.items.first().map_or(0, |it| tree.n[it.node].owner.unwrap().0)).iter().map(|&i| tree.n[i].name.as_str()).collect::<Vec<_>>().join(" > "));
            for &(a, b, how) in &w.links {
                println!("  {how:?}: {} -> {}", name(a), name(b));
            }
            for &(j, outside) in &w.sky {
                println!("  空への光({}): {}", if outside { "プログラムの外" } else { "別の世界" }, name(j));
            }
        }
    }

    App::new()
        .insert_non_send(audio)
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "fall".into(), resolution: (1440, 810).into(), ..default() }),
            ..default()
        }))
        .init_gizmo_group::<AccentLines>()
        .init_gizmo_group::<SkyLines>()
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(GlobalAmbientLight { color: Color::WHITE, brightness: AMBIENT, ..default() })
        .insert_resource(Scape(tree))
        .insert_resource(check)
        .init_resource::<Shown>()
        .add_systems(Startup, setup)
        .add_systems(Update, (script, control, travel, sound_follow, rebuild, faces, boot, draw, ground_grid, hud, dim).chain())
        .run();
}

fn setup(mut commands: Commands, scape: Res<Scape>, mut gizmo_config: ResMut<GizmoConfigStore>) {
    gizmo_config.config_mut::<DefaultGizmoConfigGroup>().0.line.width = 1.6;
    gizmo_config.config_mut::<AccentLines>().0.line.width = 3.4;
    gizmo_config.config_mut::<SkyLines>().0.line.width = 11.0;

    commands.spawn((DirectionalLight { illuminance: SUN, ..default() }, Transform::from_xyz(0.5, 0.8, 0.35).looking_at(Vec3::ZERO, Vec3::Y)));
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection { fov: FOV.to_radians(), near: 0.1, far: 6000.0, ..default() }),
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        DistanceFog { color: Color::WHITE, falloff: FogFalloff::ExponentialSquared { density: 0.01 }, ..default() },
        // 光のにじみ: 明るさが 1 を超えた所(依存の線の光)だけが、にじむ。世界の面や地の色は、にじまない
        bevy::camera::Hdr,
        Bloom { intensity: 0.5, composite_mode: BloomCompositeMode::Additive, prefilter: BloomPrefilter { threshold: 1.0, threshold_softness: 0.3 }, ..Bloom::NATURAL },
    ));
    commands.spawn((
        Text::new(""),
        TextFont { font_size: FontSize::Px(15.0), ..default() },
        Node { position_type: PositionType::Absolute, left: px(26), top: px(20), ..default() },
        Hud,
    ));
    // 照準
    commands.spawn((
        Node { position_type: PositionType::Absolute, left: Val::Percent(50.0), top: Val::Percent(50.0), width: px(6), height: px(6), margin: UiRect::all(px(-3)), ..default() },
        BackgroundColor(Color::BLACK),
        Cross,
    ));
    // 世界を渡るときに、画面を霞で覆う幕
    commands.spawn((
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        BackgroundColor(Color::NONE),
        GlobalZIndex(100),
        Veil,
    ));

    let w = scape.0.n[0].world.as_ref().expect("世界がない");
    commands.insert_resource(Nav {
        world: 0, pos: w.spawn, yaw: w.yaw, pitch: w.pitch, aim: None, tr: None, veil: 0.0, rebuild: true, changed: true, blinked: false, blink: None, born: 0.0,
        fall: 0.0, grounded: false, stride: 0.0, dip: 0.0, linked: false, stepped: false, landed: None,
    });
}

fn dive(tree: &Tree, nav: &mut Nav, item: usize) {
    let it = &tree.n[nav.world].world.as_ref().unwrap().items[item];
    let Some(inner) = tree.n[it.node].world.as_ref() else { return };
    nav.tr = Some(Tr {
        t: 0.0, switched: false, from: nav.pos, to: nav.pos + (it.pos - nav.pos).clamp_length_max(45.0), next: it.node,
        arrive: inner.spawn, drop: Vec3::Y * 46.0, yaw: inner.yaw, pitch: inner.pitch,
    });
}

fn back(tree: &Tree, nav: &mut Nav) {
    let Some((owner, item)) = tree.n[nav.world].owner else { return };
    let w = tree.n[owner].world.as_ref().unwrap();
    let it = &w.items[item];
    // 出てきた形の上に立つ。面を持たない形(点火点など)には立てないので、その世界の立つ場所に出る
    let (arrive, yaw, pitch) = if it.solid {
        let arrive = it.pos + Vec3::Y * (it.dims.y * 0.5 + EYE);
        let toward = (w.spawn - arrive).with_y(0.0).normalize_or(Vec3::Z);
        let (yaw, pitch) = world::look(arrive, arrive + toward * 10.0 - Vec3::Y * 3.0);
        (arrive, yaw, pitch)
    } else {
        (w.spawn, w.yaw, w.pitch)
    };
    nav.tr = Some(Tr { t: 0.0, switched: false, from: nav.pos, to: nav.pos + Vec3::Y * 50.0, next: owner, arrive, drop: Vec3::Y * -34.0, yaw, pitch });
}

fn facing(yaw: f32, pitch: f32) -> Quat {
    Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0)
}

/// 照準の先にあるもの(視線と、形の箱との交わり)
fn aim_at(tree: &Tree, nav: &Nav) -> Option<(usize, f32)> {
    let w = tree.n[nav.world].world.as_ref()?;
    let dir = facing(nav.yaw, nav.pitch) * Vec3::NEG_Z;
    let mut best: Option<(f32, usize)> = None;
    for (j, it) in w.items.iter().enumerate() {
        let inv = Quat::from_rotation_y(-it.yaw);
        let (o, d) = (inv * (nav.pos - it.pos), inv * dir);
        let half = it.dims * 0.5;
        let (t1, t2) = ((-half - o) / d, (half - o) / d);
        let (near, far) = (t1.min(t2).max_element(), t1.max(t2).min_element());
        if near <= far && near > 0.0 && best.is_none_or(|b| near < b.0) {
            best = Some((near, j));
        }
    }
    best.map(|b| (b.1, b.0))
}

#[allow(clippy::too_many_arguments)]
fn control(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    time: Res<Time>,
    mut cursor: Single<&mut CursorOptions>,
    scape: Res<Scape>,
    check: Res<Check>,
    mut nav: ResMut<Nav>,
) {
    let grabbed = cursor.grab_mode != CursorGrabMode::None;
    if keys.just_pressed(KeyCode::Escape) && grabbed {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
        return;
    }
    if nav.tr.is_some() {
        return;
    }
    if grabbed {
        nav.yaw -= motion.delta.x * SENS;
        nav.pitch = (nav.pitch - motion.delta.y * SENS).clamp(-1.54, 1.54);
    }
    let w = scape.0.n[nav.world].world.as_ref().unwrap();
    let forward = facing(nav.yaw, nav.pitch) * Vec3::NEG_Z;
    let right = Vec3::new(nav.yaw.cos(), 0.0, -nav.yaw.sin());
    let axis = |plus: KeyCode, minus: KeyCode| keys.pressed(plus) as i32 as f32 - keys.pressed(minus) as i32 as f32;
    let (ahead, side) = (axis(KeyCode::KeyW, KeyCode::KeyS), axis(KeyCode::KeyD, KeyCode::KeyA));
    // 移動キーが指す向き(見ている向きを含む)。ブリンクの向きに使う
    let push = (forward * ahead + right * side).normalize_or_zero();
    let dt = time.delta_secs().min(0.05);
    let crossing = nav.blink.is_some_and(|b| b.crossing());
    let solids = if check.ghost { vec![] } else { solids(w, time.elapsed_secs() - nav.born) };
    let ground = ground_of(w);
    if crossing {
        // 渡っている間は、歩けない
    } else if check.ghost {
        nav.pos += push * FLY * dt;
    } else {
        // 歩く向きは水平(上を見ても、浮かない)
        let wish = (forward.with_y(0.0).normalize_or_zero() * ahead + right * side).normalize_or_zero();
        let speed = if keys.pressed(KeyCode::ShiftLeft) { RUN } else { WALK };
        walk(&mut nav, &solids, ground, wish, speed, keys.just_pressed(KeyCode::KeyF), dt);
    }

    // Space: ブリンク。跳ぶ距離は、いつも同じ(移動キーを押していれば、その向きへ)。空中でも、何度でも使える
    if keys.just_pressed(KeyCode::Space) && !crossing {
        let (dir, len) = if push == Vec3::ZERO || push.dot(forward) > 0.999 { (forward, BLINK) } else { (push, BLINK_SIDE) };
        let to = if check.ghost { nav.pos + dir * len } else { blink_to(&solids, ground, nav.pos, dir, len) };
        // 一瞬で跳ばない。面がほどけてから、線だけの世界を渡る(travel が動かす)。落ちていた勢いは、ブリンクで消える
        nav.blink = Some(Blink { from: nav.pos, to, t: 0.0 });
        (nav.blinked, nav.grounded, nav.fall) = (true, false, 0.0);
    }

    let aim = aim_at(&scape.0, &nav).map(|h| h.0);
    if aim != nav.aim {
        nav.aim = aim;
        nav.changed = true;
    }
    let click = buttons.just_pressed(MouseButton::Left);
    if click && !grabbed {
        // 最初のクリックは、マウスをつかむだけ
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
        return;
    }
    if click || keys.just_pressed(KeyCode::KeyE) || keys.just_pressed(KeyCode::Enter) {
        if let Some(j) = nav.aim {
            dive(&scape.0, &mut nav, j);
        }
    } else if buttons.just_pressed(MouseButton::Right) || keys.just_pressed(KeyCode::KeyQ) || keys.just_pressed(KeyCode::Backspace) {
        back(&scape.0, &mut nav);
    }
}

/// ブリンクと、世界を渡る動きを進めて、視点をその位置に置く
fn travel(time: Res<Time>, check: Res<Check>, mut nav: ResMut<Nav>, mut cam: Single<&mut Transform, With<Camera3d>>) {
    let dt = time.delta_secs().min(0.05);
    // ブリンク: 面がほどける間は動かず、線だけの世界を一気に渡り、着いてから面が戻る
    if let Some(mut b) = nav.blink {
        b.t = check.freeze.unwrap_or(b.t + dt);
        if b.t < UNMAKE_SEC + CROSS_SEC + dt || check.freeze.is_some() {
            let u = ((b.t - UNMAKE_SEC) / CROSS_SEC).clamp(0.0, 1.0);
            nav.pos = b.from.lerp(b.to, u * u * (3.0 - 2.0 * u));
        }
        // 面が戻ったあとも、名前が解け終わるまでは続く
        nav.blink = (b.t < UNMAKE_SEC + CROSS_SEC + REMAKE_SEC.max(RESOLVE_SEC)).then_some(b);
    } else if check.freeze.is_some() && time.elapsed_secs() > 4.0 {
        let to = nav.pos + facing(nav.yaw, nav.pitch) * Vec3::NEG_Z * BLINK;
        nav.blink = Some(Blink { from: nav.pos, to, t: 0.0 });
    }
    if let Some(mut tr) = nav.tr.take() {
        tr.t += dt;
        if !tr.switched {
            let u = (tr.t / OUT_SEC).min(1.0);
            nav.pos = tr.from.lerp(tr.to, u * u);
            nav.veil = u;
            if u >= 1.0 {
                tr.switched = true;
                tr.t = 0.0;
                (nav.world, nav.yaw, nav.pitch) = (tr.next, tr.yaw, tr.pitch);
                nav.pos = tr.arrive + tr.drop;
                nav.aim = None;
                (nav.fall, nav.grounded, nav.dip) = (0.0, false, 0.0);
                nav.rebuild = true;
                nav.changed = true;
            }
            nav.tr = Some(tr);
        } else {
            let u = (tr.t / IN_SEC).min(1.0);
            nav.pos = (tr.arrive + tr.drop).lerp(tr.arrive, 1.0 - (1.0 - u).powi(3));
            nav.veil = (1.0 - u * 1.6).max(0.0);
            if u < 1.0 {
                nav.tr = Some(tr);
            }
        }
    }
    // 歩いている間は、視点がごく小さく上下する。着地では、沈んで戻る
    let bob = if nav.grounded { (nav.stride / STRIDE * std::f32::consts::PI).sin().abs() * BOB } else { 0.0 };
    cam.translation = nav.pos + Vec3::Y * (bob - nav.dip);
    cam.rotation = facing(nav.yaw, nav.pitch);
}

/// 世界を渡る動きに、音を合わせる。離れ始めた瞬間と、次の世界に切り替わった瞬間の 2 回だけ指示を出す
fn sound_follow(scape: Res<Scape>, mut nav: ResMut<Nav>, mut audio: NonSendMut<sound::Audio>, mut phase: Local<u8>, mut down: Local<bool>) {
    if std::mem::take(&mut nav.blinked) {
        audio.blink();
    }
    let now = match &nav.tr {
        None => 0,
        Some(tr) if !tr.switched => 1,
        Some(_) => 2,
    };
    let depth = |world: usize| scape.0.chain(world).len() - 1;
    if now == 1 && *phase != 1 {
        *down = depth(nav.tr.as_ref().unwrap().next) > depth(nav.world);
        audio.leave(*down, OUT_SEC);
    }
    if now == 2 && *phase != 2 {
        audio.arrive(depth(nav.world), *down, IN_SEC);
    }
    *phase = now;
    // 体の音: 足音、着地、落ちている間の風
    if std::mem::take(&mut nav.stepped) {
        audio.step((nav.stride / STRIDE) as u32 % 2 == 0);
    }
    if let Some(speed) = nav.landed.take() {
        audio.land(speed);
    }
    let falling = now == 0 && !nav.grounded && nav.blink.is_none_or(|b| !b.crossing());
    audio.wind(if falling { -nav.fall } else { 0.0 });
}

/// 世界が変わったら、形を作り直す。空気(色と霞)も、その世界のものにする
#[allow(clippy::too_many_arguments)]
fn rebuild(
    mut commands: Commands,
    scape: Res<Scape>,
    mut nav: ResMut<Nav>,
    mut shown: ResMut<Shown>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut fog: Single<&mut DistanceFog>,
    mut hud: Single<&mut TextColor, With<Hud>>,
    mut cross: Single<&mut BackgroundColor, With<Cross>>,
    mut cube: Local<Option<Handle<Mesh>>>,
    time: Res<Time>,
) {
    if !nav.rebuild {
        return;
    }
    nav.rebuild = false;
    nav.born = time.elapsed_secs();
    let shown = &mut *shown;
    for e in shown.slabs.drain(..).chain(shown.labels.drain(..)) {
        commands.entity(e).despawn();
    }
    let tree = &scape.0;
    let w = tree.n[nav.world].world.as_ref().unwrap();
    let air = air_of(w.law);
    // 地の色と、霞の色は、dim が毎コマ決める
    fog.falloff = FogFalloff::ExponentialSquared { density: 1.2 / w.radius };
    hud.0 = rgb(air.ink, 1.0);
    cross.0 = rgb(air.ink, 0.9);

    let cube = cube.get_or_insert_with(|| meshes.add(Cuboid::new(1.0, 1.0, 1.0))).clone();
    let mat = materials.add(StandardMaterial { base_color: rgb(air.face, 1.0), perceptual_roughness: 1.0, reflectance: 0.0, ..default() });
    for &(pos, dims, yaw) in &w.decor {
        let tf = Transform { translation: pos, rotation: Quat::from_rotation_y(yaw), scale: dims };
        shown.slabs.push(commands.spawn((Mesh3d(cube.clone()), MeshMaterial3d(mat.clone()), tf)).id());
    }
    for (j, it) in w.items.iter().enumerate() {
        if it.solid {
            for (translation, rotation, scale) in it.boxes() {
                shown.slabs.push(commands.spawn((Mesh3d(cube.clone()), MeshMaterial3d(mat.clone()), Transform { translation, rotation, scale }, Order(j))).id());
            }
        }
        shown.labels.push(commands.spawn((
            Text::new(tree.n[it.node].name.clone()),
            TextFont { font_size: FontSize::Px(14.0), ..default() },
            TextColor(rgb(air.ink, 1.0)),
            Node { position_type: PositionType::Absolute, ..default() },
            Visibility::Hidden,
            Label,
        )).id());
    }
}

#[allow(clippy::too_many_arguments)]
fn draw(
    scape: Res<Scape>,
    nav: Res<Nav>,
    shown: Res<Shown>,
    cam: Single<(&Camera, &Transform), With<Camera3d>>,
    mut labels: Query<(&mut Node, &mut Visibility, &mut TextColor, &mut Text), With<Label>>,
    mut veil: Single<&mut BackgroundColor, With<Veil>>,
    mut gizmos: Gizmos,
    mut accent: Gizmos<AccentLines>,
    mut sky: Gizmos<SkyLines>,
    time: Res<Time>,
    mut watch: Local<(Option<usize>, f32)>,
    mut cyclic: Local<Option<(usize, Vec<bool>)>>,
) {
    let (camera, cam_tf) = *cam;
    let gt = GlobalTransform::from(*cam_tf);
    let eye = cam_tf.translation;
    let tree = &scape.0;
    let w = tree.n[nav.world].world.as_ref().unwrap();
    let air = air_of(w.law);
    veil.0 = rgb(air.bg, nav.veil);
    // 線は霞の影響を受けないので、距離に応じて自分で薄くする
    let density = 1.2 / w.radius;
    let fade = |p: Vec3| haze(p.distance(eye), density);

    // 着いた瞬間、線が粗い目盛りにずれて、3 段で正しい位置に収まる(位置の情報が、粗い値から細かい値へ確定する)
    let arrived = nav.blink.map(|b| b.t - UNMAKE_SEC - CROSS_SEC);
    let grid = match arrived {
        Some(s) if (0.0..SETTLE_SEC).contains(&s) => SETTLE_GRID[(s / SETTLE_SEC * 3.0) as usize] * w.radius,
        _ => 0.0,
    };
    let fix = |p: Vec3| if grid > 0.0 { (p / grid).round() * grid } else { p };

    if let Some(b) = nav.blink {
        // 残像: 出発点から見えていた世界の輪郭が、赤い写しになって、進んだ分だけ後ろに連なる。着くと、後ろの写しから順に消えて、追いついてくる
        let power = (b.t / UNMAKE_SEC).min(1.0);
        let gone = ((b.t - UNMAKE_SEC - CROSS_SEC) / REMAKE_SEC).clamp(0.0, 1.0);
        let moved = if b.crossing() { eye - b.from } else { b.to - b.from };
        // 写しにする線: 近くの形の輪郭と、道
        let mut lines: Vec<(Vec3, Vec3, f32)> = Vec::new();
        if gone < 1.0 {
            for it in &w.items {
                if (it.pos - b.from).length() - it.dims.max_element() * 0.5 <= w.radius * 1.2 {
                    lines.extend(box_edges(it).into_iter().map(|(p, q)| (p, q, 1.0)));
                }
            }
            for path in w.paths.iter().chain(&w.thin) {
                lines.extend(path.windows(2).map(|pair| (pair[0], pair[1], 0.75)));
            }
        }
        for k in 1..=GHOSTS {
            let back = k as f32 / GHOSTS as f32;
            // 手前の写しほど濃い。着いたあとは、後ろの写しから順に消える
            let alpha = (0.18 + 0.5 * (1.0 - back)) * (((1.0 - gone) - back) * GHOSTS as f32 + 1.0).clamp(0.0, 1.0);
            if alpha <= 0.0 {
                continue;
            }
            let off = moved * back;
            for &(p, q, weight) in &lines {
                gizmos.line(p + off, q + off, rgb(ACCENT, alpha * weight * power * fade((p + q) * 0.5 + off)));
            }
        }
    }

    // 影響の範囲: 照準を合わせた形から、頼ってくる側へ、1 段ずつ赤い依存の線が伸びていく
    let now = time.elapsed_secs();
    if watch.0 != nav.aim {
        *watch = (nav.aim, now);
    }
    let since = now - watch.1;
    let hops = nav.aim.map(|j| impact(w, j));
    let hop_of = |j: usize| hops.as_ref().and_then(|h| h[j]);
    // 頼っている先: 照準を合わせた形が頼っている形。そこが変わると、この形が壊れうる。世界の線の色で、同じように 1 段ずつ伸びる
    let ups = nav.aim.map(|j| relies(w, j));
    let up_of = |j: usize| ups.as_ref().and_then(|h| h[j]);

    for (j, it) in w.items.iter().enumerate() {
        let rot = Quat::from_rotation_y(it.yaw);
        let aimed = nav.aim == Some(j);
        // 起動の途中: まだ現れていない形は描かない
        let age = now - nav.born - born_at(w, j);
        if age < 0.0 {
            if let Ok((_, mut vis, _, _)) = labels.get_mut(shown.labels[j]) {
                *vis = Visibility::Hidden;
            }
            continue;
        }
        // 現れた瞬間と、中身が入った瞬間は、赤く灯る
        let filled = now - nav.born - solid_at(w, j);
        // 影響の範囲が届いた形は、輪郭が赤くなる。頼っている先は、輪郭が太くなる
        let fresh = age < 0.35 || (it.role == Role::Filled && (0.0..0.4).contains(&filled));
        let reached = hop_of(j).is_some_and(|h| h > 0 && since > h as f32 * HOP_SEC);
        let source = up_of(j).is_some_and(|h| h > 0 && since > h as f32 * HOP_SEC);
        let half = it.dims * 0.5;
        let at = |x: f32, y: f32, z: f32| it.pos + rot * Vec3::new(x, y, z);
        let ring = |y: f32| [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0)].map(|(sx, sz)| at(half.x * sx, y, half.z * sz));
        let strength = if it.solid { 1.0 } else { 0.6 };
        for (a, b) in box_edges(it) {
            let (a, b) = (fix(a), fix(b));
            if aimed || fresh { edge(&mut accent, a, b, ACCENT, 1.0, eye, density * 0.4) } else if reached { edge(&mut gizmos, a, b, ACCENT, 0.9, eye, density * 0.5) } else if source { edge(&mut accent, a, b, air.ink, 0.9, eye, density * 0.5) } else { edge(&mut gizmos, a, b, air.ink, strength, eye, density) }
        }
        match it.role {
            // 点火点: 走った瞬間、火花が四方へ飛ぶ。そのあとは、焼けた跡(中に × のある、線だけの印)として残る
            Role::Spark => {
                if age < 0.7 {
                    let u = age / 0.7;
                    for n in 0..14 {
                        let h = (n as f32 * 12.9898 + j as f32).sin() * 43758.547;
                        let dir = Vec3::new(h.fract() - 0.0, (h * 1.7).fract(), (h * 2.3).fract()).normalize_or(Vec3::Y);
                        let (p, q) = (it.pos + dir * (4.0 + 26.0 * u * 0.6), it.pos + dir * (4.0 + 26.0 * u));
                        accent.line_gradient(p, q, rgb(ACCENT, 0.0), rgb(ACCENT, 1.0 - u));
                    }
                }
                for (sx, sz) in [(1.0, 1.0), (1.0, -1.0)] {
                    edge(&mut gizmos, fix(at(-half.x * sx, -half.y, -half.z * sz)), fix(at(half.x * sx, half.y, half.z * sz)), air.ink, 0.8, eye, density);
                }
            }
            // 書き換えられる値: 外へ広がって消える枠が、ゆっくり繰り返す(脈)
            Role::Live => {
                let beat = (now * 0.55 + j as f32 * 0.37).fract();
                let grow = 1.0 + 0.45 * beat;
                for &(p, q) in &box_edges(it) {
                    let (p, q) = (it.pos + (p - it.pos) * grow, it.pos + (q - it.pos) * grow);
                    edge(&mut gizmos, fix(p), fix(q), air.ink, 0.7 * (1.0 - beat), eye, density);
                }
            }
            _ => {}
        }
        // 中の区切りが、外から帯として見える
        for f in &it.bands {
            let c = ring(half.y - f * it.dims.y);
            for pair in c.windows(2) {
                edge(&mut gizmos, fix(pair[0]), fix(pair[1]), air.ink, 0.45, eye, density);
            }
        }
        if let Ok((mut node, mut vis, mut color, mut text)) = labels.get_mut(shown.labels[j]) {
            // ブリンクの間、名前は意味のない文字に化ける。着くと、頭から順に元の名前へ解ける
            let name = &tree.n[it.node].name;
            let want = match nav.blink {
                Some(b) => garble(name, j, b.t, arrived.unwrap_or(0.0) / RESOLVE_SEC),
                None => name.clone(),
            };
            if text.0 != want {
                text.0 = want;
            }
            // 果てなく続く形は、名前が自分の近くについてくる
            let ax = if it.dims.x > 200.0 { (eye.x - it.pos.x - 9.0).clamp(-half.x, half.x) } else { -half.x };
            let corner = fix(at(ax, half.y, half.z));
            let near = corner.distance(eye) < w.radius * 0.8;
            match camera.world_to_viewport(&gt, corner) {
                Ok(p) if aimed || near => {
                    node.left = px(p.x + 3.0);
                    node.top = px(p.y - 20.0);
                    // まだ解けていない名前は、赤い
                    let raw = nav.blink.is_some() && arrived.unwrap_or(0.0) < RESOLVE_SEC;
                    color.0 = if aimed { rgb(ACCENT, 1.0) } else if raw { rgb(ACCENT, fade(corner).max(0.55)) } else { rgb(air.ink, fade(corner)) };
                    *vis = Visibility::Visible;
                }
                _ => *vis = Visibility::Hidden,
            }
        }
    }
    for path in w.paths.iter().chain(&w.thin) {
        for pair in path.windows(2) {
            edge(&mut gizmos, fix(pair[0]), fix(pair[1]), air.ink, 0.75, eye, density);
        }
    }
    // 依存の線は、ふだんは引かない。どの世界でも、照準を合わせた形の、影響の範囲と、頼っている先だけを、光として引く
    if cyclic.as_ref().is_none_or(|c| c.0 != nav.world) {
        *cyclic = Some((nav.world, cycles(w)));
    }
    let cyclic = &cyclic.as_ref().unwrap().1;
    for (n, &(a, b, _)) in w.links.iter().enumerate() {
        let (ia, ib) = (&w.items[a], &w.items[b]);
        let age = now - nav.born - born_at(w, a).max(born_at(w, b));
        if age < 0.0 {
            continue;
        }
        let how = w.links[n].2;
        let mut arc = Arc::between(ia, ib);
        arc.dotted = how == How::Type;
        // 書く線は太い。ほかは細い
        let thick = how == How::Write;
        (arc.from, arc.peak, arc.to) = (fix(arc.from), fix(arc.peak), fix(arc.to));
        let len = arc.len;
        // 起動でつながった瞬間だけは、照準に関係なく、頼る側から相手へ、赤い光が走る。起動で埋まる値は、光が呼んだ先へ行って、戻ってくる
        if age < FILL_SEC {
            let u = age / FILL_SEC;
            let u = if ia.role == Role::Filled { 1.0 - (u * 2.0 - 1.0).abs() } else { u };
            arc.light(&mut accent, (u - 0.3).max(0.0), u, ACCENT, 0.0, 1.0, eye, density);
        }
        if cyclic[n] {
            // 循環: 照準を合わせた形から依存の線をたどって届いたときに、初めて赤く現れる(探し出すもの)。光の粒が、終わりなく回り続ける
            let seen = |j: usize| hop_of(j).or(up_of(j));
            let (Some(ha), Some(hb)) = (seen(a), seen(b)) else { continue };
            if since < ha.max(hb) as f32 * HOP_SEC {
                continue;
            }
            // 息をするように、明るさが揺れる
            let pulse = 0.6 + 0.4 * (now * 3.0).sin();
            arc.light(&mut accent, 0.0, 1.0, ACCENT, pulse, pulse, eye, density);
            let (gap, size) = (9.0, 3.5);
            let mut head = (now * 22.0) % gap;
            while head < len {
                arc.light(&mut accent, (head - size).max(0.0) / len, head / len, Vec3::new(1.0, 0.75, 0.7), 0.0, 1.0, eye, density);
                head += gap;
            }
            continue;
        }
        let phase = since * 30.0 + (a * 7 + b * 3) as f32;
        // 頼っている先への線: 頼っている側(from)から、頼られている側(to)へ、白い光が伸びる
        if let Some(h) = up_of(a) {
            let u = (since / HOP_SEC - h as f32).clamp(0.0, 1.0);
            if u > 0.0 {
                if thick { arc.light(&mut accent, 0.0, u, WHITE, 0.55, 0.55, eye, density) } else { arc.light(&mut gizmos, 0.0, u, WHITE, 0.55, 0.55, eye, density) }
                arc.motes(&mut accent, 0.0, u, WHITE, phase, how, eye, density);
            }
        }
        // 影響の範囲の線: 頼られている側(to)から、頼っている側(from)へ、赤い光が伸びる
        let Some(h) = hop_of(b) else { continue };
        let u = (since / HOP_SEC - h as f32).clamp(0.0, 1.0);
        if u <= 0.0 {
            continue;
        }
        if thick { arc.light(&mut accent, 1.0 - u, 1.0, ACCENT, 0.55, 0.55, eye, density) } else { arc.light(&mut gizmos, 1.0 - u, 1.0, ACCENT, 0.55, 0.55, eye, density) }
        arc.motes(&mut accent, 1.0 - u, 1.0, ACCENT, phase, how, eye, density);
    }
    // 空への光: 依存が、この世界の外へ続いている形から、空へ向かって太い光が立つ。照準を合わせたときと、たどって届いたときに出る
    for &(j, outside) in &w.sky {
        let reached = if nav.aim == Some(j) { Some(0) } else { up_of(j).or(hop_of(j)) };
        let Some(h) = reached else { continue };
        let u = ((since - h as f32 * HOP_SEC) / SKY_SEC).clamp(0.0, 1.0);
        if u <= 0.0 || now - nav.born < born_at(w, j) {
            continue;
        }
        let it = &w.items[j];
        let base = fix(it.pos + Vec3::Y * it.dims.y * 0.5);
        let top = base + Vec3::Y * SKY_H * u;
        // 行き先がプログラムの外(ネットワーク、ファイル、プロセス)なら、不規則に明滅する。別の世界なら、安定している
        let steady = !outside || (((now * 13.0).floor() * 12.9898 + j as f32).sin() * 43758.547).fract().abs() > 0.38;
        let power = if steady { 1.0 } else { 0.18 };
        sky.line_gradient(base, top, glow(WHITE, 0.9 * power), glow(WHITE, 0.0));
        // 光の粒は、空から形へ降りてくる(外から、ここへ届く)
        let (gap, size) = (60.0, 22.0);
        let mut head = SKY_H * u - (since * 140.0) % gap;
        while head > 0.0 {
            let fade = 1.0 - head / SKY_H;
            sky.line_gradient(base + Vec3::Y * head, base + Vec3::Y * (head + size), glow(WHITE, fade * power), glow(WHITE, 0.0));
            head -= gap;
        }
    }
}

/// 依存の線の通り道: 頼る側の天辺の縁から出て、弧を描いて、頼られる側の天辺の縁に入る(形の中は通らない)
struct Arc {
    from: Vec3,
    peak: Vec3,
    to: Vec3,
    len: f32,
    /// 点線で引くか(型としてだけ使う依存)
    dotted: bool,
}

impl Arc {
    fn between(a: &world::Item, b: &world::Item) -> Arc {
        let (from, to) = (rim(a, b.pos), rim(b, a.pos));
        let peak = (from + to) * 0.5 + Vec3::Y * ((from.y - to.y).abs() * 0.5 + from.distance(to) * ARC_LIFT);
        Arc { from, peak, to, len: (from.distance(peak) + peak.distance(to) + from.distance(to)) * 0.5, dotted: false }
    }

    /// 弧の上の点(t = 0 が頼る側、1 が頼られる側)
    fn at(&self, t: f32) -> Vec3 {
        self.from.lerp(self.peak, t).lerp(self.peak.lerp(self.to, t), t)
    }

    /// 弧の t0〜t1 を、光で引く。a0 と a1 は、両端の濃さ
    #[allow(clippy::too_many_arguments)]
    fn light<G: GizmoConfigGroup>(&self, g: &mut Gizmos<G>, t0: f32, t1: f32, color: Vec3, a0: f32, a1: f32, eye: Vec3, density: f32) {
        let steps = ((t1 - t0).abs() * 28.0).ceil().max(1.0) as usize;
        let point = |s: usize| {
            let k = s as f32 / steps as f32;
            let p = self.at(t0 + (t1 - t0) * k);
            (p, glow(color, (a0 + (a1 - a0) * k) * haze(p.distance(eye), density * 0.5)))
        };
        for s in 0..steps {
            // 点線: 弧の上の決まった位置で、1 つおきに抜く(伸びている途中でも、点の位置は動かない)
            if self.dotted && ((t0 + (t1 - t0) * s as f32 / steps as f32) * 56.0) as usize % 2 == 1 {
                continue;
            }
            let ((p, cp), (q, cq)) = (point(s), point(s + 1));
            g.line_gradient(p, q, cp, cq);
        }
    }

    /// 弧の lo〜hi の上を流れる、光の粒。依存の種類で、流れ方が違う:
    /// 書く = 値(頼られる側)へ流れ込む / 呼び出し = 行って、戻る / 型 = 粒なし / ほか = 頼られる側から、頼る側へ(影響が届く向き)
    #[allow(clippy::too_many_arguments)]
    fn motes<G: GizmoConfigGroup>(&self, g: &mut Gizmos<G>, lo: f32, hi: f32, color: Vec3, phase: f32, how: How, eye: Vec3, density: f32) {
        if how == How::Type {
            return;
        }
        let (gap, size) = (14.0, 3.0);
        let trip = if how == How::Call { self.len * 2.0 } else { self.len };
        let mut s = phase % gap;
        while s < trip {
            let (t, ahead) = match how {
                How::Write => (s / self.len, 1.0),
                How::Call if s < self.len => (s / self.len, 1.0),
                How::Call => (2.0 - s / self.len, -1.0),
                _ => (1.0 - s / self.len, -1.0),
            };
            if (lo..=hi).contains(&t) {
                self.light(g, (t - ahead * size / self.len).clamp(0.0, 1.0), t, color, 0.0, 1.0, eye, density);
            }
            s += gap;
        }
    }
}

/// 形の天辺の縁のうち、toward にいちばん近い側の点
fn rim(it: &world::Item, toward: Vec3) -> Vec3 {
    let rot = Quat::from_rotation_y(it.yaw);
    let half = it.dims * 0.5;
    let d = (rot.inverse() * (toward - it.pos)).with_y(0.0);
    let k = (d.x.abs() / half.x).max(d.z.abs() / half.z);
    let out = if k > 1e-4 { d / k } else { Vec3::ZERO };
    it.pos + rot * Vec3::new(out.x, half.y, out.z)
}

/// 依存の線が出ている間、世界を暗くする(地の色、霞、面を照らす明るさ)。光が浮かび上がる
fn dim(time: Res<Time>, scape: Res<Scape>, nav: Res<Nav>, mut clear: ResMut<ClearColor>, mut fog: Single<&mut DistanceFog>, mut ambient: ResMut<GlobalAmbientLight>, mut sun: Single<&mut DirectionalLight>, mut now: Local<f32>) {
    let want = if nav.linked && nav.tr.is_none() { 1.0 } else { 0.0 };
    *now += (want - *now) * (1.0 - (-DIM_SPEED * time.delta_secs()).exp());
    let k = 1.0 + (DIM - 1.0) * *now;
    let bg = air_of(scape.0.n[nav.world].world.as_ref().unwrap().law).bg * k;
    clear.0 = rgb(bg, 1.0);
    fog.color = rgb(bg, FOG_CAP);
    ambient.brightness = AMBIENT * k;
    sun.illuminance = SUN * k;
}

/// 化けた名前を作る。頭から solved(0〜1)の割合までは元の文字、残りは数コマごとに入れ替わる記号
fn garble(name: &str, seed: usize, t: f32, solved: f32) -> String {
    let n = name.chars().count();
    let keep = (solved.clamp(0.0, 1.0) * n as f32).ceil() as usize;
    let tick = (t / 0.034) as usize;
    name.chars()
        .enumerate()
        .map(|(i, c)| {
            if i < keep {
                return c;
            }
            let h = ((seed * 31 + i * 17 + tick * 101) as f32 * 12.9898).sin() * 43758.547;
            NOISE[(h.fract().abs() * NOISE.len() as f32) as usize % NOISE.len()] as char
        })
        .collect()
}

/// 縦坑の世界で、j 番目の形が現れる時刻(着いてからの秒)。書かれた順に、上から灯っていく。ほかの世界では、最初から全部ある
fn born_at(w: &world::World, j: usize) -> f32 {
    if w.law != Law::Shaft {
        return f32::MIN;
    }
    BOOT_WAIT + j as f32 * (BOOT_SEC / w.items.len().max(1) as f32).min(0.3)
}

/// 面が現れる時刻。起動で埋まる値は、頼っている先が全部現れて、光が行って戻るまで、空の枠のまま
fn solid_at(w: &world::World, j: usize) -> f32 {
    let born = born_at(w, j);
    if w.items[j].role != Role::Filled {
        return born;
    }
    w.links.iter().filter(|l| l.0 == j).map(|l| born_at(w, l.1)).fold(born, f32::max) + FILL_SEC
}

/// 起動の途中は、まだ現れていない形の面を隠す
fn boot(time: Res<Time>, scape: Res<Scape>, nav: Res<Nav>, mut slabs: Query<(&Order, &mut Visibility)>) {
    let w = scape.0.n[nav.world].world.as_ref().unwrap();
    let since = time.elapsed_secs() - nav.born;
    if w.law != Law::Shaft || nav.blink.is_some() || since > BOOT_WAIT + BOOT_SEC + 1.0 {
        return;
    }
    for (order, mut v) in &mut slabs {
        *v = if solid_at(w, order.0) <= since { Visibility::Inherited } else { Visibility::Hidden };
    }
}

/// 循環: 頼り合って元に戻ってくる依存の線を探す。a が b に頼り、b もたどっていくと a に頼っているなら、その線は循環の一部
fn cycles(w: &world::World) -> Vec<bool> {
    let mut reach: Vec<Option<Vec<Option<u32>>>> = vec![None; w.items.len()];
    w.links.iter().map(|&(a, b, _)| a == b || reach[a].get_or_insert_with(|| impact(w, a))[b].is_some()).collect()
}

/// 影響の範囲: この形を変えたら壊れうる形を、頼ってくる側へたどって集める。値は、何段たどった先か(自分は 0)
fn impact(w: &world::World, root: usize) -> Vec<Option<u32>> {
    spread(w, root, false)
}

/// 頼っている先: この形が頼っている形を、頼る先へたどって集める(それが変わると、この形が壊れうる)
fn relies(w: &world::World, root: usize) -> Vec<Option<u32>> {
    spread(w, root, true)
}

fn spread(w: &world::World, root: usize, down: bool) -> Vec<Option<u32>> {
    let mut hop = vec![None; w.items.len()];
    hop[root] = Some(0);
    let mut d = 0;
    loop {
        let mut grew = false;
        for &(a, b, _) in &w.links {
            let (a, b) = if down { (b, a) } else { (a, b) };
            if hop[b] == Some(d) && hop[a].is_none() {
                hop[a] = Some(d + 1);
                grew = true;
            }
        }
        if !grew {
            return hop;
        }
        d += 1;
    }
}

/// 形の輪郭(箱の 12 本の辺)。部分に分かれた形は、部分ごと
fn box_edges(it: &world::Item) -> Vec<(Vec3, Vec3)> {
    let mut out = Vec::new();
    for (c, rot, dims) in it.boxes() {
        let h = dims * 0.5;
        let loop_at = |y: f32| [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0)].map(|(sx, sz)| c + rot * Vec3::new(h.x * sx, y, h.z * sz));
        let (hi, lo) = (loop_at(h.y), loop_at(-h.y));
        for c in 0..4 {
            out.extend([(hi[c], hi[c + 1]), (lo[c], lo[c + 1]), (hi[c], lo[c])]);
        }
    }
    out
}

/// ブリンクの間、世界の面を消して、線だけの骨組みにする。押した瞬間に、手前から外へほどけ、着いた先で、手前から奥へ戻る
fn faces(scape: Res<Scape>, nav: Res<Nav>, shown: Res<Shown>, mut slabs: Query<(&Transform, &mut Visibility)>, mut whole: Local<bool>) {
    let Some(b) = nav.blink else {
        // ブリンクが終わったら、一度だけ全部を戻す
        if !*whole {
            *whole = true;
            for &e in &shown.slabs {
                if let Ok((_, mut v)) = slabs.get_mut(e) {
                    *v = Visibility::Inherited;
                }
            }
        }
        return;
    };
    *whole = false;
    let reach = scape.0.n[nav.world].world.as_ref().unwrap().radius * 1.3;
    let remake = (b.t - UNMAKE_SEC - CROSS_SEC) / REMAKE_SEC;
    for &e in &shown.slabs {
        let Ok((tf, mut v)) = slabs.get_mut(e) else { continue };
        // 大きな形は、中心ではなく表面までの近さで測る
        let near = |p: Vec3| ((tf.translation - p).length() - tf.scale.max_element() * 0.5).max(0.0);
        let show = if remake >= 0.0 { near(b.to) < remake * remake * reach } else { near(b.from) > b.t / UNMAKE_SEC * reach };
        *v = if show { Visibility::Inherited } else { Visibility::Hidden };
    }
}

/// 霞で薄くなったあとに残る濃さ(線は霞の影響を受けないので、距離に応じて自分で薄くする)
fn haze(dist: f32, density: f32) -> f32 {
    let d = dist * density;
    1.0 - (1.0 - (-d * d).exp()).min(FOG_CAP)
}

/// 線を引く。短く区切って、区切りごとに霞で薄くする
fn edge<G: GizmoConfigGroup>(g: &mut Gizmos<G>, a: Vec3, b: Vec3, ink: Vec3, alpha: f32, eye: Vec3, density: f32) {
    let fade = |p: Vec3| alpha * haze(p.distance(eye), density);
    let steps = (a.distance(b) * density / 0.2).ceil().clamp(1.0, 64.0) as usize;
    for s in 0..steps {
        let (p, q) = (a.lerp(b, s as f32 / steps as f32), a.lerp(b, (s + 1) as f32 / steps as f32));
        g.line_gradient(p, q, rgb(ink, fade(p)), rgb(ink, fade(q)));
    }
}

fn hud(scape: Res<Scape>, mut nav: ResMut<Nav>, mut text: Single<&mut Text, With<Hud>>) {
    if !nav.changed {
        return;
    }
    nav.changed = false;
    let tree = &scape.0;
    let chain = tree.chain(nav.world);
    let path: Vec<&str> = chain.iter().map(|&i| tree.n[i].name.as_str()).collect();
    let w = tree.n[nav.world].world.as_ref().unwrap();
    // 照準の先を変えたら、壊れうる形の数(影響の範囲)
    let breaks = nav.aim.map_or(0, |j| impact(w, j).iter().flatten().filter(|&&h| h > 0).count());
    let cyclic = nav.aim.is_some_and(|j| w.links.iter().zip(cycles(w)).any(|(&(a, b, _), r)| r && (a == j || b == j)));
    let needs = nav.aim.map_or(0, |j| relies(w, j).iter().flatten().filter(|&&h| h > 0).count());
    let leaves = nav.aim.and_then(|j| w.sky.iter().find(|s| s.0 == j));
    nav.linked = cyclic || breaks > 0 || needs > 0 || leaves.is_some();
    // 照準の形が、じかに頼っている先の内訳(依存の種類ごとの数)
    let kinds = [(How::Call, "calls"), (How::Read, "reads"), (How::Write, "writes"), (How::Type, "types"), (How::Import, "imports")]
        .iter()
        .filter_map(|&(how, word)| {
            let n = w.links.iter().filter(|l| Some(l.0) == nav.aim && l.2 == how).count();
            (n > 0).then(|| format!("   {word} {n}"))
        })
        .collect::<String>();
    let note = format!(
        "{}{}{}{}{}",
        if cyclic { "   !! circular dependency" } else { "" },
        if breaks > 0 { format!("   change it -> {breaks} may break") } else { String::new() },
        if needs > 0 { format!("   relies on {needs}") } else { String::new() },
        kinds,
        match leaves {
            Some((_, true)) => "   -> outside the program",
            Some(_) => "   -> another world",
            None => "",
        }
    );
    let aim = match nav.aim.map(|j| w.items[j].node) {
        Some(k) if tree.n[k].world.is_some() => format!("[ {} ]   click / E : dive{note}", tree.n[k].name),
        Some(k) => format!("[ {} ]{note}", tree.n[k].name),
        None => String::new(),
    };
    text.0 = format!(
        "{}\ndepth {}   {:?}\n{}\n\nmouse look   WASD walk   Shift run   F jump   Space blink (+ move key: that way)\nclick / E dive   right click / Q back   Esc release mouse",
        path.join("  >  "), chain.len() - 1, w.law, aim
    );
}

/// 地面を、足もとの近くだけ格子で見せる
fn ground_grid(scape: Res<Scape>, check: Res<Check>, nav: Res<Nav>, mut gizmos: Gizmos) {
    if check.ghost {
        return;
    }
    let w = scape.0.n[nav.world].world.as_ref().unwrap();
    let (y, ink) = (ground_of(w) + 0.03, air_of(w.law).ink);
    const CELL: f32 = 10.0;
    const N: i32 = 14;
    let c = Vec3::new((nav.pos.x / CELL).round() * CELL, y, (nav.pos.z / CELL).round() * CELL);
    for i in -N..=N {
        let a = 0.26 * (1.0 - i.abs() as f32 / N as f32);
        let (o, far) = (i as f32 * CELL, N as f32 * CELL);
        for (mid, end) in [(Vec3::new(o, 0.0, 0.0), Vec3::Z * far), (Vec3::new(0.0, 0.0, o), Vec3::X * far)] {
            gizmos.line_gradient(c + mid, c + mid + end, rgb(ink, a), rgb(ink, 0.0));
            gizmos.line_gradient(c + mid, c + mid - end, rgb(ink, a), rgb(ink, 0.0));
        }
    }
}

/// 確かめるための自動操作(`Check` の指定に従う): 順に潜り、視点を決め、画面を 1 枚撮って終わる。
/// step = 何番目まで潜ったか、ready = 潜り終えて視点を決めた時刻、done = 撮った時刻
#[allow(clippy::too_many_arguments)]
fn script(mut commands: Commands, time: Res<Time>, scape: Res<Scape>, check: Res<Check>, mut nav: ResMut<Nav>, mut keys: ResMut<ButtonInput<KeyCode>>, mut step: Local<usize>, mut ready: Local<Option<f32>>, mut done: Local<Option<f32>>, mut exit: MessageWriter<AppExit>) {
    let t = time.elapsed_secs();
    let tree = &scape.0;
    // 最初の 3 秒は待つ(GPU が速いと、描画の準備が終わる前に撮ってしまう)。潜っている途中は、着くまで待つ(時刻では待たない。描画が遅いと、ずれる)
    if t < 3.0 || nav.tr.is_some() {
        return;
    }
    if *step < check.go.len() {
        let w = tree.n[nav.world].world.as_ref().unwrap();
        let name = &check.go[*step];
        match w.items.iter().position(|it| tree.n[it.node].world.is_some() && tree.n[it.node].name.contains(name)) {
            Some(j) => dive(tree, &mut nav, j),
            None => eprintln!("GO: {name} が見つからない。ここにあるのは: {}", w.items.iter().filter(|it| tree.n[it.node].world.is_some()).map(|it| tree.n[it.node].name.as_str()).collect::<Vec<_>>().join(", ")),
        }
        *step += 1;
        return;
    }
    let Some(path) = check.shot.clone() else { return };
    if ready.is_none() {
        *ready = Some(t);
        if let Some(v) = &check.turn {
            (nav.yaw, nav.pitch) = (nav.yaw + v[0], nav.pitch + v[1]);
        }
        if let Some(name) = &check.aim {
            let w = tree.n[nav.world].world.as_ref().unwrap();
            match w.items.iter().find(|it| tree.n[it.node].name.contains(name)) {
                Some(it) => (nav.yaw, nav.pitch) = world::look(nav.pos, it.pos),
                None => eprintln!("AIM: {name} が見つからない"),
            }
        }
        if let Some(v) = &check.pose {
            (nav.pos, nav.yaw, nav.pitch) = (Vec3::new(v[0], v[1], v[2]), v[3], v[4]);
        }
        if check.tap {
            keys.press(KeyCode::Space);
        }
    }
    // 窓が前に出ていないと、押した状態が消えるので、毎コマ押す
    if check.hold {
        keys.press(KeyCode::KeyW);
    }
    match *done {
        None if t > ready.unwrap() + check.wait => {
            let aimed = nav.aim.map(|j| tree.n[tree.n[nav.world].world.as_ref().unwrap().items[j].node].name.clone());
            eprintln!("撮る: 位置 {:.1?}  立っている {}  照準 {:?}", nav.pos, nav.grounded, aimed);
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
            *done = Some(t);
        }
        Some(at) if t > at + 1.5 => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
