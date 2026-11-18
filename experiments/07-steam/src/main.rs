//! 実験 6(Steam との連携): Bevy の窓の中から、Steam の機能を呼べるか。
//!
//! 試す相手は、Valve が試験用に公開しているアプリ(Spacewar、番号 480)。自分のアプリの番号は、まだ持っていない。
//! 起動中は、Steam の上で「Spacewar をプレイ中」と表示される(フレンドにも見える)。
//!
//! 操作: O = オーバーレイを開く / A = 実績を 1 つ解除する / C = その実績を取り消す
use bevy::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use steamworks::Client;

const APP: u32 = 480;
/// Spacewar に用意されている実績の名前
const ACH: &str = "ACH_WIN_ONE_GAME";

#[derive(Resource)]
struct Steam {
    client: Option<Client>,
    error: String,
    overlay_open: Arc<AtomicBool>,
    _keep: Option<steamworks::CallbackHandle>,
    note: String,
}

#[derive(Component)]
struct Hud;
#[derive(Component)]
struct Spin;

fn main() {
    let overlay_open = Arc::new(AtomicBool::new(false));
    let (client, error) = match Client::init_app(APP) {
        Ok(c) => (Some(c), String::new()),
        Err(e) => (None, format!("{e:?}")),
    };
    let keep = client.as_ref().map(|c| {
        let flag = overlay_open.clone();
        c.register_callback(move |ev: steamworks::GameOverlayActivated| flag.store(ev.active, Relaxed))
    });
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin { primary_window: Some(Window { title: "steam".into(), resolution: (1280, 720).into(), ..default() }), ..default() }))
        .insert_resource(Steam { client, error, overlay_open, _keep: keep, note: String::new() })
        .add_systems(Startup, setup)
        .add_systems(Update, (pump, keys, hud, spin, report))
        .run();
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.spawn((Camera3d::default(), Transform::from_xyz(0.0, 2.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y)));
    commands.spawn((DirectionalLight::default(), Transform::from_xyz(2.0, 4.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y)));
    commands.spawn((Mesh3d(meshes.add(Cuboid::new(1.5, 1.5, 1.5))), MeshMaterial3d(materials.add(Color::srgb(0.8, 0.8, 0.85))), Spin));
    commands.spawn((Text::new(""), TextFont { font_size: FontSize::Px(18.0), ..default() }, Node { position_type: PositionType::Absolute, left: px(24), top: px(20), ..default() }, Hud));
}

/// Steam からの知らせを、毎コマ受け取る
fn pump(steam: Res<Steam>) {
    if let Some(c) = &steam.client {
        c.run_callbacks();
    }
}

fn keys(keys: Res<ButtonInput<KeyCode>>, mut steam: ResMut<Steam>) {
    let Some(c) = steam.client.clone() else { return };
    if keys.just_pressed(KeyCode::KeyO) {
        c.friends().activate_game_overlay("friends");
        steam.note = "asked Steam to open the overlay".into();
    }
    let stats = c.user_stats();
    if keys.just_pressed(KeyCode::KeyA) {
        let ok = stats.achievement(ACH).set().is_ok() && stats.store_stats().is_ok();
        steam.note = format!("unlock {ACH}: {}", if ok { "sent" } else { "failed" });
    }
    if keys.just_pressed(KeyCode::KeyC) {
        let ok = stats.achievement(ACH).clear().is_ok() && stats.store_stats().is_ok();
        steam.note = format!("clear {ACH}: {}", if ok { "sent" } else { "failed" });
    }
}

fn status(steam: &Steam) -> String {
    let Some(c) = &steam.client else {
        return format!("Steam: NOT connected\n{}\n\n(is the Steam client running and logged in?)", steam.error);
    };
    let ach = match c.user_stats().achievement(ACH).get() {
        Ok(true) => "unlocked",
        Ok(false) => "locked",
        Err(()) => "(not ready)",
    };
    format!(
        "Steam: connected   app {}\nlogged on: {}\noverlay enabled: {}\noverlay open now: {}\nachievement {ACH}: {ach}\n{}\n\nO: open overlay   A: unlock achievement   C: clear it   (Shift+Tab also opens the overlay)",
        c.utils().app_id().0,
        c.user().logged_on(),
        c.utils().is_overlay_enabled(),
        steam.overlay_open.load(Relaxed),
        steam.note,
    )
}

fn hud(steam: Res<Steam>, mut text: Single<&mut Text, With<Hud>>) {
    text.0 = status(&steam);
}

fn spin(time: Res<Time>, mut q: Query<&mut Transform, With<Spin>>) {
    for mut t in &mut q {
        t.rotate_y(time.delta_secs() * 0.8);
    }
}

/// REPORT=<秒>: その時間が経ったら、状態を出力して終わる(確かめるため)
fn report(time: Res<Time>, steam: Res<Steam>, mut exit: MessageWriter<AppExit>) {
    let Some(after) = std::env::var("REPORT").ok().and_then(|v| v.parse::<f32>().ok()) else { return };
    if time.elapsed_secs() > after {
        println!("{}", status(&steam));
        exit.write(AppExit::Success);
    }
}
