//! 段階 1 の実験 2b: 本物のコードから組み立てた世界を、世界として見せる。
//!   cargo run -p scape -- <プロジェクトのフォルダ>
//! 操作: ← → 選ぶ / Enter 潜る / Backspace 戻る / クリック 選ぶ(選んだものをもう一度クリックで潜る)
//! 自動で操作して画面を撮る: KEYS=Enter,Right,Enter SHOT=out.png cargo run -p scape -- <フォルダ>

mod analyze;
mod parse;
mod world;

use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use world::{Kind, Tree};

const BG: Color = Color::srgb(0.914, 0.914, 0.898);
const INK: Vec3 = Vec3::new(0.07, 0.07, 0.07);
const ACCENT: Vec3 = Vec3::new(0.898, 0.263, 0.165);
const FOG: f32 = 0.145; // 霞の濃さ(視点の距離に対する割合。どの深さでも、同じように遠くが霞む)
const FOG_CAP: f32 = 0.85; // 霞みきらない。遠くの巨大な構造が、薄い影として残る
const YAW: f32 = -0.5;
const DIVE_SEC: f32 = 1.25;
const PAN_SEC: f32 = 0.55;

/// 種類ごとの、視点の距離(その層の単位)と、見上げる・見下ろす角度
fn view_of(kind: Kind) -> (f32, f32) {
    match kind {
        Kind::World => (9.0, 0.42),
        Kind::File => (1.25, 0.12),
        Kind::Loop => (1.5, 0.34),
        Kind::Branch | Kind::Try => (1.15, 0.14),
        Kind::Package => (1.4, -0.1),
        _ => (1.0, 0.14),
    }
}

#[derive(Resource)]
struct Scape(Tree);

struct Tr {
    p0: DVec3, s0: f64, d0: f32, a0: f32,
    p1: DVec3, s1: f64, d1: f32, a1: f32,
    t: f32, dur: f32,
}

#[derive(Resource)]
struct Nav {
    focus: usize,
    selected: Option<usize>,
    p: DVec3,
    s: f64,
    dist: f32,
    pitch: f32,
    tr: Option<Tr>,
    mouse: Vec2,
    changed: bool,
}

#[derive(Resource)]
struct Handles {
    slabs: Vec<Option<Entity>>,
    labels: Vec<Option<Entity>>,
}

#[derive(Component)]
struct Slab;
#[derive(Component)]
struct Label;
#[derive(Component)]
struct Hud;

#[derive(Default, Reflect, GizmoConfigGroup)]
struct AccentLines;

fn main() {
    let root = std::env::args().nth(1).unwrap_or_else(|| "sample".into());
    let tree = world::build(std::path::Path::new(&root));
    println!("世界の節 {} 個", tree.n.len());

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "scape".into(), resolution: (1440, 810).into(), ..default() }),
            ..default()
        }))
        .init_gizmo_group::<AccentLines>()
        .insert_resource(ClearColor(BG))
        // 面の明るさ: 上の面は白、手前は薄い灰、横は灰(平らな面と黒い輪郭線の絵にする)
        .insert_resource(GlobalAmbientLight { color: Color::WHITE, brightness: 730.0, ..default() })
        .insert_resource(Scape(tree))
        .add_systems(Startup, setup)
        .add_systems(Update, (input, animate, draw, hud, script).chain())
        .run();
}

fn setup(
    mut commands: Commands,
    mut scape: ResMut<Scape>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut gizmo_config: ResMut<GizmoConfigStore>,
) {
    gizmo_config.config_mut::<DefaultGizmoConfigGroup>().0.line.width = 1.6;
    gizmo_config.config_mut::<AccentLines>().0.line.width = 3.2;

    let tree = &mut scape.0;
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let mut handles = Handles { slabs: vec![None; tree.n.len()], labels: vec![None; tree.n.len()] };
    for (i, n) in tree.n.iter().enumerate() {
        if n.solid {
            let mat = materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 1.0, reflectance: 0.0, ..default() });
            handles.slabs[i] = Some(commands.spawn((Mesh3d(cube.clone()), MeshMaterial3d(mat), Transform::default(), Visibility::Hidden, Slab)).id());
        }
        if !n.name.is_empty() && n.kind != Kind::Block {
            handles.labels[i] = Some(commands.spawn((
                Text::new(n.name.clone()),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(Color::srgb(INK.x, INK.y, INK.z)),
                Node { position_type: PositionType::Absolute, ..default() },
                Visibility::Hidden,
                Label,
            )).id());
        }
    }
    commands.insert_resource(handles);

    commands.spawn((DirectionalLight { illuminance: 1900.0, ..default() }, Transform::from_xyz(0.5, 0.8, 0.35).looking_at(Vec3::ZERO, Vec3::Y)));
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection { fov: 52f32.to_radians(), near: 0.02, far: 6000.0, ..default() }),
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        DistanceFog { color: BG.with_alpha(FOG_CAP), falloff: FogFalloff::ExponentialSquared { density: FOG }, ..default() },
    ));
    commands.spawn((
        Text::new(""),
        TextFont { font_size: FontSize::Px(15.0), ..default() },
        TextColor(Color::srgb(INK.x, INK.y, INK.z)),
        Node { position_type: PositionType::Absolute, left: px(26), top: px(20), ..default() },
        Hud,
    ));

    // 最初は世界の全体。一番上の層の、一番大きいファイルを選んでおく
    let first = tree.n[0].kids.first().copied();
    let mut nav = Nav { focus: 0, selected: first, p: DVec3::ZERO, s: 1.0, dist: 9.0, pitch: 0.42, tr: None, mouse: Vec2::ZERO, changed: true };
    set_focus(tree, &mut nav, 0, None);
    if let Some(tr) = nav.tr.take() {
        (nav.p, nav.s, nav.dist, nav.pitch) = (tr.p1, tr.s1, tr.d1, tr.a1);
    }
    commands.insert_resource(nav);
}

/// 視点は、選んだものの近くにいる。全体を収めない
fn aim(tree: &Tree, nav: &mut Nav, dur: f32) {
    let (vr, pitch) = view_of(tree.n[nav.focus].kind);
    let s1 = tree.n[nav.focus].sc;
    let (p1, d1) = match nav.selected {
        Some(k) => {
            let n = &tree.n[k];
            let extent = n.dims.max_element() * (n.sc / s1) as f32;
            (n.p, vr.max(extent * 1.15))
        }
        None => (tree.n[nav.focus].p, vr),
    };
    nav.tr = Some(Tr { p0: nav.p, s0: nav.s, d0: nav.dist, a0: nav.pitch, p1, s1, d1, a1: pitch, t: 0.0, dur });
    nav.changed = true;
}

fn set_focus(tree: &mut Tree, nav: &mut Nav, focus: usize, keep: Option<usize>) {
    let chain = tree.chain(focus);
    for n in tree.n.iter_mut() {
        n.on_chain = false;
    }
    for &c in &chain {
        tree.n[c].on_chain = true;
    }
    if let Some(k) = keep {
        tree.n[focus].sel = tree.n[focus].kids.iter().position(|&x| x == k).unwrap_or(0);
    }
    nav.focus = focus;
    nav.selected = tree.n[focus].kids.get(tree.n[focus].sel).copied();
    aim(tree, nav, DIVE_SEC);
}

fn select(tree: &mut Tree, nav: &mut Nav, index: i32) {
    let count = tree.n[nav.focus].kids.len() as i32;
    if count == 0 {
        return;
    }
    let i = index.rem_euclid(count) as usize;
    tree.n[nav.focus].sel = i;
    nav.selected = Some(tree.n[nav.focus].kids[i]);
    aim(tree, nav, PAN_SEC);
}

fn dive(tree: &mut Tree, nav: &mut Nav) {
    if let Some(k) = nav.selected {
        if !tree.n[k].kids.is_empty() {
            set_focus(tree, nav, k, None);
        }
    }
}

fn rise(tree: &mut Tree, nav: &mut Nav) {
    if let Some(p) = tree.n[nav.focus].parent {
        let from = nav.focus;
        set_focus(tree, nav, p, Some(from));
    }
}

fn press(tree: &mut Tree, nav: &mut Nav, key: &str) {
    let sel = tree.n[nav.focus].sel as i32;
    match key {
        "Right" | "Down" => select(tree, nav, sel + 1),
        "Left" | "Up" => select(tree, nav, sel - 1),
        "Enter" => dive(tree, nav),
        "Back" => rise(tree, nav),
        _ => {}
    }
}

fn input(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    cam: Single<(&Camera, &Transform), With<Camera3d>>,
    mut scape: ResMut<Scape>,
    mut nav: ResMut<Nav>,
) {
    let tree = &mut scape.0;
    for (code, name) in [
        (KeyCode::ArrowRight, "Right"), (KeyCode::ArrowDown, "Down"), (KeyCode::ArrowLeft, "Left"), (KeyCode::ArrowUp, "Up"),
        (KeyCode::Enter, "Enter"), (KeyCode::Backspace, "Back"), (KeyCode::Escape, "Back"),
    ] {
        if keys.just_pressed(code) {
            press(tree, &mut nav, name);
        }
    }
    if buttons.just_pressed(MouseButton::Right) {
        rise(tree, &mut nav);
    }
    let Some(cursor) = window.cursor_position() else { return };
    nav.mouse = Vec2::new(cursor.x / window.width() * 2.0 - 1.0, -(cursor.y / window.height() * 2.0 - 1.0));
    if buttons.just_pressed(MouseButton::Left) && nav.tr.is_none() {
        // カーソルの真下にあるものを選ぶ(視線と、形の箱との交わりで決める)。選んであるものなら、潜る
        let (camera, cam_tf) = *cam;
        let gt = GlobalTransform::from(*cam_tf);
        let Ok(ray) = camera.viewport_to_world(&gt, cursor) else { return };
        let (origin, dir) = (ray.origin, ray.direction.as_vec3());
        let mut best: Option<(f32, usize)> = None;
        for (j, &k) in tree.n[nav.focus].kids.iter().enumerate() {
            let n = &tree.n[k];
            let center = ((n.p - nav.p) / nav.s).as_vec3();
            let half = n.dims * (n.sc / nav.s) as f32 * 0.5;
            let (lo, hi) = (center - half, center + half);
            let (t1, t2) = ((lo - origin) / dir, (hi - origin) / dir);
            let near = t1.min(t2).max_element();
            let far = t1.max(t2).min_element();
            if near <= far && far > 0.0 && best.is_none_or(|b| near < b.0) {
                best = Some((near, j));
            }
        }
        if let Some((_, j)) = best {
            if Some(tree.n[nav.focus].kids[j]) == nav.selected {
                dive(tree, &mut nav);
            } else {
                select(tree, &mut nav, j as i32);
            }
        }
    }
}

fn ease(t: f32) -> f32 {
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

fn animate(time: Res<Time>, mut scape: ResMut<Scape>, mut nav: ResMut<Nav>, mut cam: Single<(&mut Transform, &mut DistanceFog), With<Camera3d>>) {
    let dt = time.delta_secs().min(0.05);
    if let Some(mut tr) = nav.tr.take() {
        tr.t += dt;
        let u = ease((tr.t / tr.dur).min(1.0));
        // 縮尺は等比で変える。拡大の中心を動かさないので、切り替えなしで中へ入っていく
        let s = tr.s0 * (tr.s1 / tr.s0).powf(u as f64);
        let f = if (tr.s0 - tr.s1).abs() < 1e-12 * tr.s0 { u as f64 } else { (tr.s0 - s) / (tr.s0 - tr.s1) };
        nav.s = s;
        nav.p = tr.p0 + (tr.p1 - tr.p0) * f;
        nav.dist = tr.d0 + (tr.d1 - tr.d0) * u;
        nav.pitch = tr.a0 + (tr.a1 - tr.a0) * u;
        if tr.t < tr.dur {
            nav.tr = Some(tr);
        }
    }
    let selected = nav.selected;
    for (i, n) in scape.0.n.iter_mut().enumerate() {
        n.open += ((n.on_chain as i32 as f32) - n.open) * (dt * 3.2).min(1.0);
        n.sel_f += (((Some(i) == selected) as i32 as f32) - n.sel_f) * (dt * 10.0).min(1.0);
    }
    // マウスでは視点を動かさない(クリックで選ぶ操作と混ざるため)。ごくゆっくり揺れるだけ
    let yaw = YAW + (time.elapsed_secs() / 5.2).sin() * 0.03;
    let pitch = nav.pitch;
    let (tf, fog) = &mut *cam;
    tf.translation = Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos()) * nav.dist;
    tf.look_at(Vec3::ZERO, Vec3::Y);
    fog.falloff = FogFalloff::ExponentialSquared { density: FOG / nav.dist };
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[allow(clippy::too_many_arguments)]
fn draw(
    mut scape: ResMut<Scape>,
    nav: Res<Nav>,
    handles: Res<Handles>,
    cam: Single<(&Camera, &Transform), With<Camera3d>>,
    mut slabs: Query<(&mut Transform, &mut Visibility, &MeshMaterial3d<StandardMaterial>), (With<Slab>, Without<Label>, Without<Camera3d>)>,
    mut labels: Query<(&mut Node, &mut Visibility, &mut TextColor), (With<Label>, Without<Slab>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut gizmos: Gizmos,
    mut accent: Gizmos<AccentLines>,
) {
    let (camera, cam_tf) = *cam;
    let gt = GlobalTransform::from(*cam_tf);
    let eye = cam_tf.translation;
    // 線は霞の影響を受けないので、距離に応じて自分で薄くする
    let fade = |p: Vec3| {
        let d = p.distance(eye) * FOG / nav.dist;
        1.0 - (1.0 - (-d * d).exp()).min(FOG_CAP)
    };
    for (_, mut v, _) in &mut slabs {
        *v = Visibility::Hidden;
    }
    for (_, mut v, _) in &mut labels {
        *v = Visibility::Hidden;
    }

    let tree = &mut scape.0;
    let mut stack = vec![0usize];
    while let Some(i) = stack.pop() {
        let n = &mut tree.n[i];
        let rel = (n.sc / nav.s) as f32;
        let apparent = rel * n.dims.max_element() / nav.dist; // 画面の中での見かけの大きさ
        if apparent < 0.012 && n.kind != Kind::World {
            continue;
        }
        let center = ((n.p - nav.p) / nav.s).as_vec3();
        let size = n.dims * rel;
        let appear = smooth(0.012, 0.03, apparent);
        let haze = fade(center);

        if n.solid {
            let alpha = appear * (1.0 - n.open);
            if alpha > 0.02 {
                if let Some(Ok((mut tf, mut vis, mat))) = handles.slabs[i].map(|e| slabs.get_mut(e)) {
                    *tf = Transform::from_translation(center).with_scale(size);
                    *vis = Visibility::Visible;
                    if (alpha - n.shown_alpha).abs() > 0.004 {
                        n.shown_alpha = alpha;
                        if let Some(mut m) = materials.get_mut(&mat.0) {
                            m.base_color = Color::WHITE.with_alpha(alpha);
                            m.alpha_mode = if alpha > 0.99 { AlphaMode::Opaque } else { AlphaMode::Blend };
                        }
                    }
                }
            }
        }
        let strength = if n.solid { 1.0 - 0.5 * n.open } else { n.frame };
        if strength > 0.0 && n.kind != Kind::World {
            let tf = Transform::from_translation(center).with_scale(size);
            let c = INK.lerp(ACCENT, n.sel_f);
            let color = Color::srgba(c.x, c.y, c.z, appear * strength.max(n.sel_f) * haze);
            if n.sel_f > 0.5 { accent.cube(tf, color) } else { gizmos.cube(tf, color) }
        }
        if !n.paths.is_empty() {
            let a = smooth(0.08, 0.25, apparent) * 0.8;
            if a > 0.01 {
                for path in &n.paths {
                    for pair in path.windows(2) {
                        let (p0, p1) = (center + pair[0] * rel, center + pair[1] * rel);
                        gizmos.line(p0, p1, Color::srgba(INK.x, INK.y, INK.z, a * fade((p0 + p1) * 0.5)));
                    }
                }
            }
        }
        // 名前を出すのは、いま中にいる層のものだけ
        if n.parent == Some(nav.focus) && n.open < 0.5 {
            if let Some(Ok((mut node, mut vis, mut color))) = handles.labels[i].map(|e| labels.get_mut(e)) {
                let corner = center + Vec3::new(-size.x * 0.5, size.y * 0.5, size.z * 0.5);
                if let Ok(p) = camera.world_to_viewport(&gt, corner) {
                    node.left = px(p.x + 2.0);
                    node.top = px(p.y - 20.0);
                    let c = INK.lerp(ACCENT, n.sel_f);
                    color.0 = Color::srgba(c.x, c.y, c.z, (1.0 - n.open) * haze.max(n.sel_f));
                    *vis = Visibility::Visible;
                }
            }
        }
        // 閉じた形の中は描かない
        if n.solid && n.open < 0.02 {
            continue;
        }
        stack.extend(n.kids.iter().copied());
    }

    // 頼っている相手へ伸びる線: 柱の底から、相手の天辺へ
    let world_apparent = (tree.n[0].sc / nav.s) as f32;
    if world_apparent < 400.0 {
        for &(a, b, mine) in &tree.links {
            let (na, nb) = (&tree.n[a], &tree.n[b]);
            let from = ((na.p - nav.p) / nav.s).as_vec3() - Vec3::Y * na.dims.y * (na.sc / nav.s) as f32 * 0.5;
            let to = ((nb.p - nav.p) / nav.s).as_vec3() + Vec3::Y * nb.dims.y * (nb.sc / nav.s) as f32 * 0.5;
            let strength = if mine { 0.8 } else { 0.3 };
            gizmos.line(from, to, Color::srgba(INK.x, INK.y, INK.z, strength * fade((from + to) * 0.5)));
        }
    }
}

fn hud(scape: Res<Scape>, mut nav: ResMut<Nav>, mut text: Single<&mut Text, With<Hud>>) {
    if !nav.changed {
        return;
    }
    nav.changed = false;
    let tree = &scape.0;
    let mut parts: Vec<String> = tree.chain(nav.focus).iter().map(|&i| tree.n[i].name.clone()).collect();
    if let Some(k) = nav.selected {
        parts.push(format!("[ {} ]", tree.n[k].name));
    }
    text.0 = format!("{}\ndepth {}", parts.join("  >  "), tree.n[nav.focus].depth);
}

/// KEYS と SHOT が指定されていたら、自動で操作して、画面を 1 枚撮って終わる(確かめるため)
fn script(mut commands: Commands, time: Res<Time>, mut scape: ResMut<Scape>, mut nav: ResMut<Nav>, mut step: Local<usize>, mut done: Local<Option<f32>>, mut exit: MessageWriter<AppExit>) {
    let Ok(path) = std::env::var("SHOT") else { return };
    let keys: Vec<String> = std::env::var("KEYS").unwrap_or_default().split(',').filter(|s| !s.is_empty()).map(str::to_string).collect();
    let t = time.elapsed_secs();
    // 最初の 3 秒は待つ(GPU が速いと、描画の準備が終わる前に撮ってしまう)
    if *step < keys.len() {
        if t > 3.0 + *step as f32 * 1.7 {
            press(&mut scape.0, &mut nav, &keys[*step]);
            *step += 1;
        }
        return;
    }
    let after = std::env::var("WAIT").ok().and_then(|v| v.parse().ok()).unwrap_or(2.2);
    match *done {
        None if t > 3.0 + keys.len() as f32 * 1.7 + after => {
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
            *done = Some(t);
        }
        Some(at) if t > at + 1.5 => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
