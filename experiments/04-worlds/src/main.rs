//! 段階 1 の実験 2c: 潜った先は、外の形に閉じ込められていない、別の世界。
//!   cargo run -p worlds -- <プロジェクトのフォルダ>
//! 操作(一人称視点のゲームと同じ):
//!   マウス 見回す / W S 見ている向きへ前後 / A D 横へ / Shift 速く / Space 照準の先へ瞬間移動
//!   左クリック・E 照準の先に潜る / 右クリック・Q 上の世界へ戻る / Esc マウスを放す
//! 自動で操作して画面を撮る:
//!   GO=server.ts,handle SHOT=out.png cargo run -p worlds -- <フォルダ>   (名前の一部で、順に潜る)
//!   POSE=x,y,z,yaw,pitch で、撮る前の視点を指定できる

mod analyze;
mod parse;
mod world;

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::{CursorGrabMode, CursorOptions};
use world::{Law, Tree};

const ACCENT: Vec3 = Vec3::new(0.898, 0.263, 0.165);
const FOG_CAP: f32 = 0.88; // 霞みきらない。遠くの巨大な構造が、薄い影として残る
const SPEED: f32 = 16.0;
const FAST: f32 = 4.0;
const BLINK: f32 = 80.0;
const BLINK_SIDE: f32 = 40.0;
const SENS: f32 = 0.0022;
const OUT_SEC: f32 = 0.5;
const IN_SEC: f32 = 1.1;

/// 世界の法則ごとの、空気(地の色、線の色、面の色)
struct Air {
    bg: Vec3,
    ink: Vec3,
    face: Vec3,
}

fn air_of(law: Law) -> Air {
    match law {
        Law::Pillars => Air { bg: Vec3::new(0.914, 0.914, 0.898), ink: Vec3::new(0.07, 0.07, 0.07), face: Vec3::ONE },
        Law::Strata => Air { bg: Vec3::new(0.76, 0.82, 0.86), ink: Vec3::new(0.05, 0.10, 0.17), face: Vec3::new(0.95, 0.98, 1.0) },
        Law::Road => Air { bg: Vec3::new(0.93, 0.86, 0.76), ink: Vec3::new(0.17, 0.09, 0.05), face: Vec3::new(1.0, 0.97, 0.91) },
        Law::Ring => Air { bg: Vec3::new(0.055, 0.065, 0.085), ink: Vec3::new(0.86, 0.89, 0.92), face: Vec3::new(0.17, 0.19, 0.23) },
    }
}

fn rgb(v: Vec3, a: f32) -> Color {
    Color::srgba(v.x, v.y, v.z, a)
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
}

#[derive(Resource, Default)]
struct Shown {
    slabs: Vec<Entity>,
    labels: Vec<Entity>,
}

#[derive(Component)]
struct Label;
#[derive(Component)]
struct Hud;
#[derive(Component)]
struct Veil;
#[derive(Component)]
struct Cross;

#[derive(Default, Reflect, GizmoConfigGroup)]
struct AccentLines;

fn main() {
    let root = std::env::args().nth(1).unwrap_or_else(|| "sample".into());
    let tree = world::build(std::path::Path::new(&root));
    println!("節 {} 個、世界 {} 個", tree.n.len(), tree.n.iter().filter(|n| n.world.is_some()).count());

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "worlds".into(), resolution: (1440, 810).into(), ..default() }),
            ..default()
        }))
        .init_gizmo_group::<AccentLines>()
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(GlobalAmbientLight { color: Color::WHITE, brightness: 730.0, ..default() })
        .insert_resource(Scape(tree))
        .init_resource::<Shown>()
        .add_systems(Startup, setup)
        .add_systems(Update, (control, travel, rebuild, draw, hud, script).chain())
        .run();
}

fn setup(mut commands: Commands, scape: Res<Scape>, mut gizmo_config: ResMut<GizmoConfigStore>) {
    gizmo_config.config_mut::<DefaultGizmoConfigGroup>().0.line.width = 1.6;
    gizmo_config.config_mut::<AccentLines>().0.line.width = 3.4;

    commands.spawn((DirectionalLight { illuminance: 1900.0, ..default() }, Transform::from_xyz(0.5, 0.8, 0.35).looking_at(Vec3::ZERO, Vec3::Y)));
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection { fov: 62f32.to_radians(), near: 0.1, far: 6000.0, ..default() }),
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        DistanceFog { color: Color::WHITE, falloff: FogFalloff::ExponentialSquared { density: 0.01 }, ..default() },
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
    commands.insert_resource(Nav { world: 0, pos: w.spawn, yaw: w.yaw, pitch: w.pitch, aim: None, tr: None, veil: 0.0, rebuild: true, changed: true });
}

fn dive(tree: &Tree, nav: &mut Nav, item: usize) {
    let it = &tree.n[nav.world].world.as_ref().unwrap().items[item];
    let Some(inner) = tree.n[it.node].world.as_ref() else { return };
    nav.tr = Some(Tr {
        t: 0.0, switched: false, from: nav.pos, to: nav.pos + (it.pos - nav.pos).clamp_length_max(45.0), next: it.node,
        arrive: inner.spawn, drop: Vec3::Y * 46.0, yaw: inner.yaw, pitch: inner.pitch,
    });
}

fn rise(tree: &Tree, nav: &mut Nav) {
    let Some((owner, item)) = tree.n[nav.world].owner else { return };
    let w = tree.n[owner].world.as_ref().unwrap();
    let it = &w.items[item];
    // 出てきた入口の前に立つ
    let mut side = w.spawn - it.pos;
    side.y = 0.0;
    let arrive = it.door.unwrap_or(it.pos + side.normalize_or(Vec3::Z) * (it.dims.max_element() * 0.9 + 12.0) + Vec3::Y * (it.dims.y * 0.2 + 2.0));
    let (yaw, pitch) = world::look(arrive, if it.door.is_some() { arrive + Vec3::new(0.3, -0.25, -1.0) } else { it.pos });
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

fn control(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    time: Res<Time>,
    mut cursor: Single<&mut CursorOptions>,
    scape: Res<Scape>,
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
    // W と S は、見ている向きへ進む(上を見て進めば、上がる)。A と D は水平に横へ
    let forward = facing(nav.yaw, nav.pitch) * Vec3::NEG_Z;
    let right = Vec3::new(nav.yaw.cos(), 0.0, -nav.yaw.sin());
    let axis = |plus: KeyCode, minus: KeyCode| keys.pressed(plus) as i32 as f32 - keys.pressed(minus) as i32 as f32;
    let step = forward * axis(KeyCode::KeyW, KeyCode::KeyS) + right * axis(KeyCode::KeyD, KeyCode::KeyA);
    let speed = SPEED * if keys.pressed(KeyCode::ShiftLeft) { FAST } else { 1.0 };
    let dt = time.delta_secs().min(0.05);
    nav.pos += step.normalize_or_zero() * speed * dt;

    let hit = aim_at(&scape.0, &nav);
    // Space: 瞬間移動。移動キーを押していれば、その向きへ。押していなければ(または前だけなら)、照準の先のすぐ手前へ
    if keys.just_pressed(KeyCode::Space) {
        let dir = step.normalize_or_zero();
        if dir == Vec3::ZERO || dir.dot(forward) > 0.999 {
            nav.pos += forward * hit.map_or(BLINK, |h| (h.1 - 6.0).max(0.0));
        } else {
            nav.pos += dir * BLINK_SIDE;
        }
    }
    let aim = hit.map(|h| h.0);
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
        rise(&scape.0, &mut nav);
    }
}

fn travel(time: Res<Time>, mut nav: ResMut<Nav>, mut cam: Single<&mut Transform, With<Camera3d>>) {
    let dt = time.delta_secs().min(0.05);
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
    cam.translation = nav.pos;
    cam.rotation = facing(nav.yaw, nav.pitch);
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
    mut clear: ResMut<ClearColor>,
    mut fog: Single<&mut DistanceFog>,
    mut hud: Single<&mut TextColor, With<Hud>>,
    mut cross: Single<&mut BackgroundColor, With<Cross>>,
    mut cube: Local<Option<Handle<Mesh>>>,
) {
    if !nav.rebuild {
        return;
    }
    nav.rebuild = false;
    let shown = &mut *shown;
    for e in shown.slabs.drain(..).chain(shown.labels.drain(..)) {
        commands.entity(e).despawn();
    }
    let tree = &scape.0;
    let w = tree.n[nav.world].world.as_ref().unwrap();
    let air = air_of(w.law);
    clear.0 = rgb(air.bg, 1.0);
    fog.color = rgb(air.bg, FOG_CAP);
    fog.falloff = FogFalloff::ExponentialSquared { density: 1.2 / w.radius };
    hud.0 = rgb(air.ink, 1.0);
    cross.0 = rgb(air.ink, 0.9);

    let cube = cube.get_or_insert_with(|| meshes.add(Cuboid::new(1.0, 1.0, 1.0))).clone();
    let mat = materials.add(StandardMaterial { base_color: rgb(air.face, 1.0), perceptual_roughness: 1.0, reflectance: 0.0, ..default() });
    for &(pos, dims, yaw) in &w.decor {
        let tf = Transform { translation: pos, rotation: Quat::from_rotation_y(yaw), scale: dims };
        shown.slabs.push(commands.spawn((Mesh3d(cube.clone()), MeshMaterial3d(mat.clone()), tf)).id());
    }
    for it in &w.items {
        let rot = Quat::from_rotation_y(it.yaw);
        let whole = [(Vec3::ZERO, it.dims)];
        for &(off, dims) in if it.parts.is_empty() { &whole[..] } else { &it.parts[..] } {
            if it.solid {
                let tf = Transform { translation: it.pos + rot * off, rotation: rot, scale: dims };
                shown.slabs.push(commands.spawn((Mesh3d(cube.clone()), MeshMaterial3d(mat.clone()), tf)).id());
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
    mut labels: Query<(&mut Node, &mut Visibility, &mut TextColor), With<Label>>,
    mut veil: Single<&mut BackgroundColor, With<Veil>>,
    mut gizmos: Gizmos,
    mut accent: Gizmos<AccentLines>,
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
    let fade = |p: Vec3| {
        let d = p.distance(eye) * density;
        1.0 - (1.0 - (-d * d).exp()).min(FOG_CAP)
    };

    for (j, it) in w.items.iter().enumerate() {
        let rot = Quat::from_rotation_y(it.yaw);
        let aimed = nav.aim == Some(j);
        let half = it.dims * 0.5;
        let at = |x: f32, y: f32, z: f32| it.pos + rot * Vec3::new(x, y, z);
        let ring = |y: f32| [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0)].map(|(sx, sz)| at(half.x * sx, y, half.z * sz));
        let strength = if it.solid { 1.0 } else { 0.6 };
        let whole = [(Vec3::ZERO, it.dims)];
        for &(off, dims) in if it.parts.is_empty() { &whole[..] } else { &it.parts[..] } {
            let h = dims * 0.5;
            let loop_at = |y: f32| [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0)].map(|(sx, sz)| it.pos + rot * (off + Vec3::new(h.x * sx, y, h.z * sz)));
            let (hi, lo) = (loop_at(h.y), loop_at(-h.y));
            for c in 0..4 {
                for (a, b) in [(hi[c], hi[c + 1]), (lo[c], lo[c + 1]), (hi[c], lo[c])] {
                    if aimed { edge(&mut accent, a, b, ACCENT, 1.0, eye, density * 0.4) } else { edge(&mut gizmos, a, b, air.ink, strength, eye, density) }
                }
            }
        }
        // 中の区切りが、外から帯として見える
        for f in &it.bands {
            let c = ring(half.y - f * it.dims.y);
            for pair in c.windows(2) {
                edge(&mut gizmos, pair[0], pair[1], air.ink, 0.45, eye, density);
            }
        }
        if let Ok((mut node, mut vis, mut color)) = labels.get_mut(shown.labels[j]) {
            // 果てなく続く形は、名前が自分の近くについてくる
            let ax = if it.dims.x > 200.0 { (eye.x - it.pos.x - 9.0).clamp(-half.x, half.x) } else { -half.x };
            let corner = at(ax, half.y, half.z);
            let near = corner.distance(eye) < w.radius * 0.8;
            match camera.world_to_viewport(&gt, corner) {
                Ok(p) if aimed || near => {
                    node.left = px(p.x + 3.0);
                    node.top = px(p.y - 20.0);
                    color.0 = if aimed { rgb(ACCENT, 1.0) } else { rgb(air.ink, fade(corner)) };
                    *vis = Visibility::Visible;
                }
                _ => *vis = Visibility::Hidden,
            }
        }
    }
    for path in w.paths.iter().chain(&w.thin) {
        for pair in path.windows(2) {
            edge(&mut gizmos, pair[0], pair[1], air.ink, 0.75, eye, density);
        }
    }
    // 頼っている相手へ伸びる線: 柱の底から、相手の天辺へ
    for &(a, b, mine) in &w.links {
        let (ia, ib) = (&w.items[a], &w.items[b]);
        let from = ia.pos - Vec3::Y * ia.dims.y * 0.5;
        let to = ib.pos + Vec3::Y * ib.dims.y * 0.5;
        edge(&mut gizmos, from, to, air.ink, if mine { 0.8 } else { 0.3 }, eye, density);
    }
}

/// 線を引く。線は霞の影響を受けないので、短く区切って、距離に応じて自分で薄くする
fn edge<G: GizmoConfigGroup>(g: &mut Gizmos<G>, a: Vec3, b: Vec3, ink: Vec3, alpha: f32, eye: Vec3, density: f32) {
    let fade = |p: Vec3| {
        let d = p.distance(eye) * density;
        alpha * (1.0 - (1.0 - (-d * d).exp()).min(FOG_CAP))
    };
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
    let aim = match nav.aim.map(|j| w.items[j].node) {
        Some(k) if tree.n[k].world.is_some() => format!("[ {} ]   click / E : dive", tree.n[k].name),
        Some(k) => format!("[ {} ]", tree.n[k].name),
        None => String::new(),
    };
    text.0 = format!(
        "{}\ndepth {}   {:?}\n{}\n\nmouse look   WASD move (W goes where you look)   Shift fast   Space blink (+ move key: that way)\nclick / E dive   right click / Q back   Esc release mouse",
        path.join("  >  "), chain.len() - 1, w.law, aim
    );
}

/// GO と SHOT が指定されていたら、自動で潜って、画面を 1 枚撮って終わる(確かめるため)
#[allow(clippy::too_many_arguments)]
fn script(mut commands: Commands, time: Res<Time>, scape: Res<Scape>, mut nav: ResMut<Nav>, mut step: Local<usize>, mut posed: Local<bool>, mut done: Local<Option<f32>>, mut exit: MessageWriter<AppExit>) {
    let Ok(path) = std::env::var("SHOT") else { return };
    let go: Vec<String> = std::env::var("GO").unwrap_or_default().split(',').filter(|s| !s.is_empty()).map(str::to_string).collect();
    let t = time.elapsed_secs();
    // 最初の 3 秒は待つ(GPU が速いと、描画の準備が終わる前に撮ってしまう)
    if *step < go.len() {
        if t > 3.0 + *step as f32 * 2.2 {
            let tree = &scape.0;
            let w = tree.n[nav.world].world.as_ref().unwrap();
            match w.items.iter().position(|it| tree.n[it.node].world.is_some() && tree.n[it.node].name.contains(&go[*step])) {
                Some(j) => dive(tree, &mut nav, j),
                None => eprintln!("GO: {} が見つからない", go[*step]),
            }
            *step += 1;
        }
        return;
    }
    let ready = 3.0 + go.len() as f32 * 2.2;
    if !*posed && t > ready {
        *posed = true;
        if let Ok(pose) = std::env::var("POSE") {
            let v: Vec<f32> = pose.split(',').filter_map(|s| s.trim().parse().ok()).collect();
            if v.len() == 5 {
                (nav.pos, nav.yaw, nav.pitch) = (Vec3::new(v[0], v[1], v[2]), v[3], v[4]);
            }
        }
    }
    let after = std::env::var("WAIT").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    match *done {
        None if t > ready + after => {
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
            *done = Some(t);
        }
        Some(at) if t > at + 1.5 => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
