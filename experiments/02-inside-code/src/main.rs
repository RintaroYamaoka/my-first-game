//! 段階 1 の実験 2: 本物の構文解析でコードの中を読み、関数・文・分岐・ループまで世界にする。
//!   cargo run -p inside-code -- <ファイル...>
//! 操作: ← → 回る / ↑ ↓ 上下 / W S 近づく・離れる
//! 画面を撮って終わる: SHOT=out.png ANGLE=0.5 HEIGHT=-2 DIST=9 cargo run -p inside-code -- <ファイル...>

mod layout;
mod parse;

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use layout::Scene;
use parse::Kind;

const BG: Color = Color::srgb(0.914, 0.914, 0.898);

#[derive(Resource)]
struct World3(Scene);

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
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("使い方: inside-code <ファイル...>");
        return;
    }
    let mut files = vec![];
    for a in &args {
        let path = std::path::Path::new(a);
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| a.clone());
        match parse::parse_file(path, name) {
            Ok(n) => {
                parse::report(&n, 0);
                files.push(n);
            }
            Err(e) => eprintln!("{e}"),
        }
    }
    let scene = layout::build(&files);
    let lowest = scene.items.iter().map(|i| i.center.y - i.size.y / 2.0).fold(0.0, f32::min);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "inside code".into(), resolution: (1440, 810).into(), ..default() }),
            ..default()
        }))
        .insert_resource(ClearColor(BG))
        .insert_resource(GlobalAmbientLight { color: Color::WHITE, brightness: 700.0, ..default() })
        .insert_resource(World3(scene))
        .insert_resource(Orbit {
            angle: env_f32("ANGLE", 0.5),
            height: env_f32("HEIGHT", lowest / 2.0),
            dist: env_f32("DIST", 11.0),
            auto: std::env::var("SHOT").is_err(),
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (orbit, outlines, labels, shot).chain())
        .run();
}

fn setup(mut commands: Commands, world: Res<World3>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let white = materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 1.0, reflectance: 0.0, ..default() });
    for (i, item) in world.0.items.iter().enumerate() {
        if item.solid {
            commands.spawn((Mesh3d(cube.clone()), MeshMaterial3d(white.clone()), Transform::from_translation(item.center).with_scale(item.size)));
        }
        commands.spawn((
            Text::new(item.label.clone()),
            TextFont { font_size: FontSize::Px(13.0), ..default() },
            TextColor(Color::srgb(0.07, 0.07, 0.07)),
            Node { position_type: PositionType::Absolute, ..default() },
            Visibility::Hidden,
            Label(i),
        ));
    }
    commands.spawn((DirectionalLight { illuminance: 9000.0, ..default() }, Transform::from_xyz(4.0, 8.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y)));
    commands.spawn((
        Camera3d::default(),
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        DistanceFog { color: BG, falloff: FogFalloff::ExponentialSquared { density: 0.025 }, ..default() },
    ));
}

fn orbit(time: Res<Time>, keys: Res<ButtonInput<KeyCode>>, mut o: ResMut<Orbit>, mut cam: Single<&mut Transform, With<Camera3d>>) {
    let dt = time.delta_secs();
    if o.auto {
        o.angle += dt * 0.04;
    }
    let pressed = |k| keys.pressed(k) as i32 as f32;
    o.angle += (pressed(KeyCode::ArrowRight) - pressed(KeyCode::ArrowLeft)) * dt * 0.9;
    o.height += (pressed(KeyCode::ArrowUp) - pressed(KeyCode::ArrowDown)) * dt * 4.0;
    o.dist = (o.dist + (pressed(KeyCode::KeyS) - pressed(KeyCode::KeyW)) * dt * 7.0).max(1.5);
    let target = Vec3::new(0.0, o.height, 0.0);
    cam.translation = target + Vec3::new(o.angle.sin() * o.dist, o.dist * 0.22, o.angle.cos() * o.dist);
    cam.look_at(target, Vec3::Y);
}

fn outlines(world: Res<World3>, cam: Single<&Transform, With<Camera3d>>, mut gizmos: Gizmos) {
    let ink = |p: Vec3, strength: f32| {
        let d = p.distance(cam.translation) * 0.025;
        let fog = 1.0 - (-d * d).exp();
        Color::srgba(0.07, 0.07, 0.07, strength * (1.0 - fog.min(0.85)))
    };
    for item in &world.0.items {
        let strength = match (item.solid, item.kind) {
            (true, _) => 1.0,
            (false, Kind::File) => 0.55,
            (false, Kind::Type) => 0.6,
            (false, Kind::Loop) => continue, // 輪は、流れの線で描く
            (false, _) => 0.28,
        };
        gizmos.cube(Transform::from_translation(item.center).with_scale(item.size), ink(item.center, strength));
    }
    for flow in &world.0.flows {
        for pair in flow.windows(2) {
            gizmos.line(pair[0], pair[1], ink((pair[0] + pair[1]) * 0.5, 0.75));
        }
    }
}

fn labels(world: Res<World3>, cam: Single<(&Camera, &GlobalTransform)>, mut q: Query<(&mut Node, &mut Visibility, &Label)>) {
    let (camera, cam_tf) = *cam;
    let right = cam_tf.right().as_vec3();
    for (mut node, mut vis, label) in &mut q {
        let item = &world.0.items[label.0];
        let top = item.center + Vec3::Y * (item.size.y * 0.5);
        let a = camera.world_to_viewport(cam_tf, top - right * 0.5);
        let b = camera.world_to_viewport(cam_tf, top + right * 0.5);
        // 画面の中で十分に大きいものだけ、名前を出す
        let (Ok(a), Ok(b)) = (a, b) else { *vis = Visibility::Hidden; continue };
        let need = if matches!(item.kind, Kind::File | Kind::Fn | Kind::Loop) { 55.0 } else { 150.0 };
        if (b.x - a.x).abs() < need {
            *vis = Visibility::Hidden;
            continue;
        }
        let corner = item.center + Vec3::new(-item.size.x * 0.5, item.size.y * 0.5, item.size.z * 0.5);
        if let Ok(p) = camera.world_to_viewport(cam_tf, corner) {
            node.left = px(p.x + 4.0);
            node.top = px(p.y - 17.0);
            *vis = Visibility::Visible;
        }
    }
}

fn shot(mut commands: Commands, time: Res<Time>, mut taken: Local<bool>, mut exit: MessageWriter<AppExit>) {
    let Ok(path) = std::env::var("SHOT") else { return };
    // 枚数ではなく時間で待つ(GPU が速いと、描画の準備が終わる前に規定の枚数に届く)
    let t = time.elapsed_secs();
    if t > 3.0 && !*taken {
        *taken = true;
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
    if t > 4.5 {
        exit.write(AppExit::Success);
    }
}
