//! 段階 1 の実験 1: 本物のプロジェクトを読み取り、世界として表示する。
//!   cargo run -- <プロジェクトのフォルダ>      (省略すると sample)
//! 操作: ← → 回る / ↑ ↓ 上下 / W S 近づく・離れる
//! 画面を撮って終わる: SHOT=out.png ANGLE=0.6 HEIGHT=1 DIST=14 cargo run

mod analyze;

use analyze::{Kind, World};
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

const BG: Color = Color::srgb(0.914, 0.914, 0.898);
const LAYER_GAP: f32 = 2.6; // 層と層の間
const GROUND_GAP: f32 = 1.2; // 自分のコードと、依存パッケージの間のすき間

#[derive(Resource)]
struct Scape {
    world: World,
    /// 柱ごとの、中心と大きさ
    boxes: Vec<(Vec3, Vec3)>,
}

#[derive(Resource)]
struct Orbit {
    angle: f32,
    height: f32,
    dist: f32,
    auto: bool,
}

#[derive(Component)]
struct Label(usize);

fn env_f32(key: &str, default: f32) -> f32 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn main() {
    let root = std::env::args().nth(1).unwrap_or_else(|| "sample".into());
    let world = analyze::analyze(std::path::Path::new(&root));
    analyze::report(&world);

    // 形と大きさは、必ず本物の性質から決める
    // 自分のコードの真ん中を、世界の中心にする
    let files: Vec<&analyze::Node> = world.nodes.iter().filter(|n| n.kind == Kind::File).collect();
    let cx = files.iter().map(|n| n.x).sum::<f32>() / files.len().max(1) as f32;
    let cz = files.iter().map(|n| n.z).sum::<f32>() / files.len().max(1) as f32;
    let boxes: Vec<(Vec3, Vec3)> = world.nodes.iter().map(|n| match n.kind {
        Kind::File => {
            let h = 0.25 + n.size as f32 * 0.07; // 高さ = 行数
            (Vec3::new(n.x - cx, n.layer as f32 * LAYER_GAP, n.z - cz), Vec3::new(0.42, h, 0.42))
        }
        Kind::Package => {
            let f = (n.size as f32 + 1.0).ln(); // 大きさ = 中のファイル数(対数)
            let y = -GROUND_GAP + n.layer as f32 * LAYER_GAP;
            (Vec3::new(n.x - cx, y, n.z - cz), Vec3::new(0.25 + f * 0.09, 0.2 + f * 0.33, 0.25 + f * 0.09))
        }
    }).collect();

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "world from code".into(), resolution: (1440, 810).into(), ..default() }),
            ..default()
        }))
        .insert_resource(ClearColor(BG))
        .insert_resource(GlobalAmbientLight { color: Color::WHITE, brightness: 1150.0, ..default() })
        .insert_resource(Scape { world, boxes })
        .insert_resource(Orbit {
            angle: env_f32("ANGLE", 0.6),
            height: env_f32("HEIGHT", 1.5),
            dist: env_f32("DIST", 15.0),
            auto: std::env::var("SHOT").is_err(),
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (orbit, outlines, labels, shot).chain())
        .run();
}

fn setup(
    mut commands: Commands,
    scape: Res<Scape>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let white = materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 1.0, reflectance: 0.0, ..default() });
    for (center, size) in &scape.boxes {
        commands.spawn((Mesh3d(cube.clone()), MeshMaterial3d(white.clone()), Transform::from_translation(*center).with_scale(*size)));
    }
    commands.spawn((
        DirectionalLight { illuminance: 6500.0, ..default() },
        Transform::from_xyz(5.0, 8.0, 3.5).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Camera3d::default(),
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        // 遠くは霞む
        DistanceFog { color: BG, falloff: FogFalloff::ExponentialSquared { density: 0.03 }, ..default() },
    ));
    // 名前を出すのは、自分のコードと、自分で頼んだパッケージだけ
    for (i, n) in scape.world.nodes.iter().enumerate() {
        if n.kind == Kind::File || n.direct {
            commands.spawn((
                Text::new(n.name.clone()),
                TextFont { font_size: FontSize::Px(15.0), ..default() },
                TextColor(Color::srgb(0.07, 0.07, 0.07)),
                Node { position_type: PositionType::Absolute, ..default() },
                Label(i),
            ));
        }
    }
}

fn orbit(time: Res<Time>, keys: Res<ButtonInput<KeyCode>>, mut o: ResMut<Orbit>, mut cam: Single<&mut Transform, With<Camera3d>>) {
    let dt = time.delta_secs();
    if o.auto {
        o.angle += dt * 0.05;
    }
    let pressed = |k| keys.pressed(k) as i32 as f32;
    o.angle += (pressed(KeyCode::ArrowRight) - pressed(KeyCode::ArrowLeft)) * dt * 0.9;
    o.height += (pressed(KeyCode::ArrowUp) - pressed(KeyCode::ArrowDown)) * dt * 5.0;
    o.dist = (o.dist + (pressed(KeyCode::KeyS) - pressed(KeyCode::KeyW)) * dt * 8.0).max(2.0);
    let target = Vec3::new(0.0, o.height, 0.0);
    cam.translation = target + Vec3::new(o.angle.sin() * o.dist, o.dist * 0.16, o.angle.cos() * o.dist);
    cam.look_at(target, Vec3::Y);
}

/// 黒い輪郭線と、頼っている相手へ伸びる線
fn outlines(scape: Res<Scape>, cam: Single<&Transform, With<Camera3d>>, mut gizmos: Gizmos) {
    // 線は霞の影響を受けないので、距離に応じて自分で薄くする
    let ink = |p: Vec3, strength: f32| {
        let d = p.distance(cam.translation) * 0.03;
        let fog = 1.0 - (-d * d).exp();
        Color::srgba(0.07, 0.07, 0.07, strength * (1.0 - fog.min(0.85)))
    };
    for (center, size) in &scape.boxes {
        gizmos.cube(Transform::from_translation(*center).with_scale(*size), ink(*center, 1.0));
    }
    for (i, n) in scape.world.nodes.iter().enumerate() {
        let (a, sa) = scape.boxes[i];
        for &d in &n.deps {
            let (b, sb) = scape.boxes[d];
            // 柱の底から、頼っている相手の天辺へ
            let from = a - Vec3::Y * sa.y * 0.5;
            let to = b + Vec3::Y * sb.y * 0.5;
            let strength = if n.kind == Kind::File { 0.9 } else { 0.35 };
            gizmos.line(from, to, ink((from + to) * 0.5, strength));
        }
    }
}

fn labels(scape: Res<Scape>, cam: Single<(&Camera, &GlobalTransform)>, mut q: Query<(&mut Node, &mut Visibility, &Label)>) {
    let (camera, cam_tf) = *cam;
    for (mut node, mut vis, label) in &mut q {
        let (center, size) = scape.boxes[label.0];
        let top = center + Vec3::new(-size.x * 0.5, size.y * 0.5 + 0.12, size.z * 0.5);
        match camera.world_to_viewport(cam_tf, top) {
            Ok(p) => {
                node.left = px(p.x);
                node.top = px(p.y - 18.0);
                *vis = Visibility::Visible;
            }
            Err(_) => *vis = Visibility::Hidden,
        }
    }
}

/// SHOT が指定されていたら、画面を 1 枚撮って終わる(自動で確かめるため)
fn shot(mut commands: Commands, mut frame: Local<u32>, mut exit: MessageWriter<AppExit>) {
    let Ok(path) = std::env::var("SHOT") else { return };
    *frame += 1;
    if *frame == 40 {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
    if *frame == 80 {
        exit.write(AppExit::Success);
    }
}
