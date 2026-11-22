//! 実験 7(音を重ねる): 一つの曲から分けたパートを、ずれなく重ねる・抜く・こもらせる、が kira でできるか。
//!
//! 使い方: layers <パートの入ったフォルダ>   (synth.flac, bass.flac, ... を読む)
//!
//! 操作: 1〜7 = パートを入れる・抜く / 0 = 全部入れる・全部抜く / B = 小節の頭に合わせるかを切り替える
//!       ↑↓ = 深さ(深いほど、こもって響く) / R = 最初から
use bevy::prelude::*;
use kira::clock::{ClockHandle, ClockSpeed, ClockTime};
use kira::effect::filter::{FilterBuilder, FilterHandle, FilterMode};
use kira::effect::reverb::{ReverbBuilder, ReverbHandle};
use kira::sound::FromFileError;
use kira::sound::streaming::{StreamingSoundData, StreamingSoundHandle};
use kira::track::{TrackBuilder, TrackHandle};
use kira::{AudioManager, AudioManagerSettings, DefaultBackend, Easing, Mix, StartTime, Tween};
use std::path::PathBuf;
use std::time::Duration;

/// 曲の速さ(1 分あたりの拍)。Suno に頼んだ値で、測った値も同じだった
const BPM: f64 = 100.0;
const BEATS_PER_BAR: u64 = 4;
/// 曲が始まる拍(時計の 0 拍目は、時計が動き出す前に過ぎてしまうので 1 拍目から)
const START_TICK: u64 = 1;
/// 重ねる順。下から積み上がるように並べた
const NAMES: [&str; 7] = ["synth", "bass", "drums", "percussion", "keyboard", "strings", "fx"];
/// パートを全部足すと元の曲より 2dB ほど大きく、頭が割れるので、全体を下げる
const MASTER_DB: f32 = -4.0;
const ON_DB: f32 = 0.0;
const OFF_DB: f32 = -60.0;

struct Layer {
    name: &'static str,
    handle: StreamingSoundHandle<FromFileError>,
    on: bool,
}

struct Audio {
    manager: AudioManager,
    track: TrackHandle,
    filter: FilterHandle,
    reverb: ReverbHandle,
    clock: ClockHandle,
    layers: Vec<Layer>,
    dir: PathBuf,
}

#[derive(Resource)]
struct State {
    /// 0 = 浅い(そのままの音)、1 = 深い(こもって響く)
    depth: f32,
    /// 入れる・抜くを、次の小節の頭まで待つか
    on_bar: bool,
    note: String,
}

#[derive(Component)]
struct Hud;

fn main() {
    let dir = PathBuf::from(std::env::args().nth(1).expect("パートの入ったフォルダを指定する"));
    let audio = build(dir).expect("音の準備に失敗");
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin { primary_window: Some(Window { title: "layers".into(), resolution: (900, 560).into(), ..default() }), ..default() }))
        .insert_non_send(audio)
        .insert_resource(State { depth: 0.0, on_bar: true, note: String::new() })
        .insert_resource(ClearColor(Color::srgb(0.02, 0.03, 0.05)))
        .add_systems(Startup, setup)
        .add_systems(Update, (keys, hud, report))
        .run();
}

/// 音の出口、こもらせる・響かせる仕組み、拍の時計を作り、全パートを同じ拍から鳴らし始める
fn build(dir: PathBuf) -> Result<Audio, Box<dyn std::error::Error>> {
    let mut manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())?;
    let mut builder = TrackBuilder::new().volume(MASTER_DB);
    let filter = builder.add_effect(FilterBuilder::new().mode(FilterMode::LowPass).cutoff(20000.0));
    let reverb = builder.add_effect(ReverbBuilder::new().feedback(0.85).damping(0.3).mix(Mix(0.0)));
    let track = manager.add_sub_track(builder)?;
    let clock = manager.add_clock(ClockSpeed::TicksPerMinute(BPM))?;
    let mut audio = Audio { manager, track, filter, reverb, clock, layers: Vec::new(), dir };
    start(&mut audio, &[true, false, false, false, false, false, false])?;
    Ok(audio)
}

/// 全パートを、時計の同じ拍に予約して鳴らす(同じ拍に予約した音は、同じサンプルから始まる)
fn start(audio: &mut Audio, on: &[bool]) -> Result<(), Box<dyn std::error::Error>> {
    for l in &mut audio.layers {
        l.handle.stop(Tween::default());
    }
    audio.layers.clear();
    audio.clock.stop();
    let at = ClockTime { clock: audio.clock.id(), ticks: START_TICK, fraction: 0.0 };
    for (i, name) in NAMES.iter().enumerate() {
        let path = audio.dir.join(format!("{name}.flac"));
        if !path.exists() {
            continue;
        }
        let is_on = on.get(i).copied().unwrap_or(false);
        let data = StreamingSoundData::from_file(&path)?.start_time(at).volume(if is_on { ON_DB } else { OFF_DB });
        let handle = audio.track.play(data)?;
        audio.layers.push(Layer { name, handle, on: is_on });
    }
    audio.clock.start();
    Ok(())
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((Text::new(""), TextFont { font_size: FontSize::Px(20.0), ..default() }, Node { position_type: PositionType::Absolute, left: px(28), top: px(24), ..default() }, Hud));
}

/// 入れる・抜くの音量の動き。小節の頭に合わせるなら、次の小節の頭から 1 拍かけて動かす
fn fade(audio: &Audio, on_bar: bool) -> Tween {
    let beat = Duration::from_secs_f64(60.0 / BPM);
    let start_time = if on_bar {
        let now = audio.clock.time().ticks;
        let into = now.saturating_sub(START_TICK) % BEATS_PER_BAR;
        StartTime::ClockTime(ClockTime { clock: audio.clock.id(), ticks: now + (BEATS_PER_BAR - into), fraction: 0.0 })
    } else {
        StartTime::Immediate
    };
    Tween { start_time, duration: beat, easing: Easing::Linear }
}

fn keys(keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, mut audio: NonSendMut<Audio>, mut state: ResMut<State>) {
    let digits = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7];
    let tween = fade(&audio, state.on_bar);
    for (i, key) in digits.iter().enumerate() {
        if keys.just_pressed(*key) {
            if let Some(l) = audio.layers.get_mut(i) {
                l.on = !l.on;
                l.handle.set_volume(if l.on { ON_DB } else { OFF_DB }, tween);
            }
        }
    }
    if keys.just_pressed(KeyCode::Digit0) {
        let all = !audio.layers.iter().all(|l| l.on);
        for l in &mut audio.layers {
            l.on = all;
            l.handle.set_volume(if all { ON_DB } else { OFF_DB }, tween);
        }
    }
    if keys.just_pressed(KeyCode::KeyB) {
        state.on_bar = !state.on_bar;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        let on: Vec<bool> = NAMES.iter().map(|n| audio.layers.iter().any(|l| l.name == *n && l.on)).collect();
        state.note = match start(&mut audio, &on) {
            Ok(()) => "restarted".into(),
            Err(e) => format!("restart failed: {e}"),
        };
    }
    let dir = keys.pressed(KeyCode::ArrowDown) as i32 - keys.pressed(KeyCode::ArrowUp) as i32;
    if dir != 0 {
        state.depth = (state.depth + dir as f32 * time.delta_secs() * 0.4).clamp(0.0, 1.0);
        let quick = Tween { duration: Duration::from_millis(50), ..default() };
        audio.filter.set_cutoff(cutoff(state.depth), quick);
        audio.reverb.set_mix(Mix(state.depth * 0.5), quick);
    }
}

/// 深さからこもり具合へ。浅い = 20000Hz(そのまま)、深い = 300Hz(壁の向こうのような音)
fn cutoff(depth: f32) -> f64 {
    20000.0 * (300.0f64 / 20000.0).powf(depth as f64)
}

/// 各パートの再生位置のずれ(いちばん進んでいるものと、いちばん遅れているものの差。秒)
fn drift(audio: &Audio) -> f64 {
    let pos: Vec<f64> = audio.layers.iter().map(|l| l.handle.position()).collect();
    pos.iter().cloned().fold(f64::MIN, f64::max) - pos.iter().cloned().fold(f64::MAX, f64::min)
}

fn status(audio: &Audio, state: &State) -> String {
    let t = audio.clock.time();
    let beat = t.ticks.saturating_sub(START_TICK);
    let mut s = format!("bar {:>3}  beat {}   position {:6.1}s   drift {:.4}s\n\n", beat / BEATS_PER_BAR + 1, beat % BEATS_PER_BAR + 1, audio.layers.first().map(|l| l.handle.position()).unwrap_or(0.0), drift(audio));
    for (i, l) in audio.layers.iter().enumerate() {
        s += &format!("  {}  [{}]  {}\n", i + 1, if l.on { "ON " } else { "   " }, l.name);
    }
    s += &format!("\ndepth {:.2}   lowpass {:>5.0} Hz   reverb {:.2}\n", state.depth, cutoff(state.depth), state.depth * 0.5);
    s += &format!("change on: {}\n{}\n\n", if state.on_bar { "next bar" } else { "immediately" }, state.note);
    s += "1-7: layer on/off   0: all on/off   B: bar sync on/off\nUp/Down: depth   R: restart";
    s
}

fn hud(audio: NonSend<Audio>, state: Res<State>, mut text: Single<&mut Text, With<Hud>>) {
    text.0 = status(&audio, &state);
}

/// REPORT=<秒>: その時間が経ったら、状態を出力して終わる(確かめるため)。途中で全パートを入れる
fn report(time: Res<Time>, mut audio: NonSendMut<Audio>, state: Res<State>, mut exit: MessageWriter<AppExit>, mut done: Local<bool>) {
    let Some(after) = std::env::var("REPORT").ok().and_then(|v| v.parse::<f32>().ok()) else { return };
    if !*done && time.elapsed_secs() > after / 2.0 {
        *done = true;
        let tween = fade(&audio, true);
        for l in &mut audio.layers {
            l.on = true;
            l.handle.set_volume(ON_DB, tween);
        }
    }
    if time.elapsed_secs() > after {
        println!("{}", status(&audio, &state));
        let _ = &audio.manager;
        exit.write(AppExit::Success);
    }
}
