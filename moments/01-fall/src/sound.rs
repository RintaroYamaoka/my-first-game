//! 音: 曲のパートを、深さに合わせて抜く・こもらせる・響かせる。潜る・戻る瞬間の音は、コードで作る。
//!
//! 曲は、一つの曲から分けたパート(Suno で作って書き出した物)。全パートを同じ拍から鳴らし続け、音量だけを動かす。
use kira::clock::{ClockHandle, ClockSpeed, ClockTime};
use kira::effect::filter::{FilterBuilder, FilterHandle, FilterMode};
use kira::effect::reverb::{ReverbBuilder, ReverbHandle};
use kira::sound::FromFileError;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings};
use kira::sound::streaming::{StreamingSoundData, StreamingSoundHandle};
use kira::track::{TrackBuilder, TrackHandle};
use kira::{AudioManager, AudioManagerSettings, DefaultBackend, Easing, Frame, Mix, Tween};
use std::f32::consts::TAU;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

const BPM: f64 = 100.0;
const NAMES: [&str; 7] = ["synth", "bass", "drums", "percussion", "keyboard", "strings", "fx"];
/// パートを全部足すと頭が割れるので、全体を下げる
const MUSIC_DB: f32 = -11.0;
/// 効果音の大きさ。曲より前に出す(2026-11-21 本人: 効果音が BGM に比べて小さい)
const SFX_DB: f32 = 0.0;
/// ブリンクの音だけ、少し下げる(何度も続けて鳴るので)
const BLINK_DB: f32 = -4.0;
const OFF_DB: f32 = -60.0;
/// 足音の大きさ(2026-11-22 本人: -16 は「小さすぎ」)
const STEP_DB: f32 = -6.0;
/// 着地の音: いちばん小さいときの大きさ、落ちてきた速さあたりで大きくなる分、上限
const LAND_DB: f32 = -14.0;
const LAND_DB_PER_SPEED: f32 = 0.4;
const LAND_DB_MAX: f32 = -2.0;
/// 落ちている間の風: 鳴り始める速さ、そのときの大きさ、速さあたりで大きくなる分、上限
const WIND_FROM: f32 = 8.0;
const WIND_DB: f32 = -30.0;
const WIND_DB_PER_SPEED: f32 = 0.35;
const WIND_DB_MAX: f32 = -8.0;
const RATE: u32 = 48000;

/// 深さごとの音の姿。調整するのは、この表
/// (鳴らすパート, こもり具合 Hz, 響きの量)。深いほどパートが減り、こもって、響く
const DEPTHS: [(&[&str], f64, f32); 4] = [
    (&["synth", "bass", "drums", "percussion", "keyboard", "strings", "fx"], 20000.0, 0.04),
    (&["synth", "bass", "drums", "keyboard", "strings"], 5000.0, 0.16),
    (&["synth", "bass", "keyboard"], 1800.0, 0.30),
    (&["synth", "bass"], 700.0, 0.45),
];
/// 世界を渡っている間の、こもり具合(ほとんど聞こえないところまで絞る)
const VEIL_HZ: f64 = 180.0;

pub struct Audio {
    _manager: AudioManager,
    _music: TrackHandle,
    sfx: TrackHandle,
    filter: FilterHandle,
    reverb: ReverbHandle,
    _clock: ClockHandle,
    layers: Vec<(&'static str, StreamingSoundHandle<FromFileError>)>,
    fall: StaticSoundData,
    rise: StaticSoundData,
    /// 潜って、次の世界に着いたときの低い音
    arrived: StaticSoundData,
    /// 体が落ちてきて、着地したときの音
    landing: StaticSoundData,
    blink: StaticSoundData,
    step: StaticSoundData,
    /// 落ちている間の風(鳴らしっぱなしで、大きさだけを動かす)と、今の大きさ
    wind: StaticSoundHandle,
    wind_db: f32,
}

fn tween(sec: f32) -> Tween {
    Tween { duration: Duration::from_secs_f32(sec), easing: Easing::Linear, ..Default::default() }
}

fn look(depth: usize) -> (&'static [&'static str], f64, f32) {
    DEPTHS[depth.min(DEPTHS.len() - 1)]
}

impl Audio {
    /// 曲のフォルダが無ければ、曲なしで、潜る・戻る音だけ鳴らす
    pub fn new(music_dir: Option<&Path>, out_sec: f32, in_sec: f32) -> Result<Audio, Box<dyn std::error::Error>> {
        let mut manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())?;
        let (on, hz, wet) = look(0);
        let mut builder = TrackBuilder::new().volume(MUSIC_DB);
        let filter = builder.add_effect(FilterBuilder::new().mode(FilterMode::LowPass).cutoff(hz));
        let reverb = builder.add_effect(ReverbBuilder::new().feedback(0.86).damping(0.35).mix(Mix(wet)));
        let mut music = manager.add_sub_track(builder)?;
        let mut sfx = manager.add_sub_track(TrackBuilder::new().volume(SFX_DB).with_effect(ReverbBuilder::new().feedback(0.8).damping(0.5).mix(Mix(0.2))))?;
        let mut clock = manager.add_clock(ClockSpeed::TicksPerMinute(BPM))?;
        let at = ClockTime { clock: clock.id(), ticks: 1, fraction: 0.0 };
        let mut layers = Vec::new();
        if let Some(dir) = music_dir {
            for name in NAMES {
                let path = dir.join(format!("{name}.flac"));
                if !path.exists() {
                    continue;
                }
                let data = StreamingSoundData::from_file(&path)?.start_time(at).loop_region(0.0..).volume(if on.contains(&name) { 0.0 } else { OFF_DB });
                layers.push((name, music.play(data)?));
            }
        }
        clock.start();
        let (fall, rise, arrived, blink) = (sweep(out_sec + in_sec, true), sweep(out_sec + in_sec, false), thump(), blink());
        let (step, landing, wind) = (step(), landing(), wind());
        // SFX_DUMP=<フォルダ>: 作った効果音を WAV で書き出す(単体で聞く・測るため)
        if let Ok(dir) = std::env::var("SFX_DUMP") {
            for (name, d) in [("fall", &fall), ("rise", &rise), ("arrived", &arrived), ("blink", &blink), ("step", &step), ("landing", &landing), ("wind", &wind)] {
                write_wav(&Path::new(&dir).join(format!("{name}.wav")), d)?;
            }
        }
        let wind = sfx.play(wind.loop_region(..).volume(OFF_DB))?;
        Ok(Audio {
            _manager: manager,
            _music: music,
            sfx,
            filter,
            reverb,
            _clock: clock,
            layers,
            fall,
            rise,
            arrived,
            landing,
            blink: blink.volume(BLINK_DB),
            step: step.volume(STEP_DB),
            wind,
            wind_db: OFF_DB,
        })
    }

    pub fn has_music(&self) -> bool {
        !self.layers.is_empty()
    }

    /// ブリンクした瞬間
    pub fn blink(&mut self) {
        let _ = self.sfx.play(self.blink.clone());
    }

    /// 一歩。左右の足で、少しだけ高さを変える
    pub fn step(&mut self, left: bool) {
        let _ = self.sfx.play(self.step.playback_rate(if left { 1.0 } else { 0.92 }));
    }

    /// 落ちてきて、着地した瞬間。速いほど大きい
    pub fn land(&mut self, speed: f32) {
        let db = (LAND_DB + speed * LAND_DB_PER_SPEED).min(LAND_DB_MAX);
        let _ = self.sfx.play(self.landing.volume(db));
    }

    /// 落ちている間の風。速いほど大きい(speed が小さければ、消える)
    pub fn wind(&mut self, speed: f32) {
        let db = if speed < WIND_FROM { OFF_DB } else { (WIND_DB + (speed - WIND_FROM) * WIND_DB_PER_SPEED).min(WIND_DB_MAX) };
        if (db - self.wind_db).abs() > 0.5 {
            self.wind_db = db;
            self.wind.set_volume(db, tween(0.15));
        }
    }

    /// 世界を離れ始めた瞬間: 曲が遠のき、落ちる(または浮かぶ)音が鳴る
    pub fn leave(&mut self, down: bool, out_sec: f32) {
        self.filter.set_cutoff(VEIL_HZ, tween(out_sec));
        let _ = self.sfx.play(if down { self.fall.clone() } else { self.rise.clone() });
    }

    /// 次の世界に切り替わった瞬間: その深さの音の姿へ、着くまでの時間をかけて変わる
    pub fn arrive(&mut self, depth: usize, down: bool, in_sec: f32) {
        let (on, hz, wet) = look(depth);
        self.filter.set_cutoff(hz, tween(in_sec));
        self.reverb.set_mix(Mix(wet), tween(in_sec));
        for (name, handle) in &mut self.layers {
            handle.set_volume(if on.contains(name) { 0.0 } else { OFF_DB }, tween(in_sec));
        }
        if down {
            // 着いた音は、落ちきる少し前に鳴り始める
            let _ = self.sfx.play(self.arrived.start_time(Duration::from_secs_f32(in_sec * 0.55)));
        }
    }
}

/// 音を作る道具: 雑音(左右で別の種)
struct Noise(u32);
impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}

/// 音を作る道具: 高さを動かせる、癖の強いフィルター。(低い側, 真ん中, 高い側) を返す
#[derive(Default)]
struct Svf(f32, f32);
impl Svf {
    fn run(&mut self, x: f32, hz: f32, q: f32) -> (f32, f32, f32) {
        let g = (std::f32::consts::PI * (hz / RATE as f32).min(0.45)).tan();
        let k = 1.0 / q;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let v3 = x - self.1;
        let v1 = a1 * self.0 + g * a1 * v3;
        let v2 = self.1 + g * a1 * self.0 + g * g * a1 * v3;
        self.0 = 2.0 * v1 - self.0;
        self.1 = 2.0 * v2 - self.1;
        (v2, v1, x - k * v1 - v2)
    }
}

/// a から b へ、比で滑らかに動く値(u は 0〜1)
fn glide(a: f32, b: f32, u: f32) -> f32 {
    a * (b / a).powf(u.clamp(0.0, 1.0))
}

/// 1 サンプルずつ作って、いちばん大きい所が 0.9 になるように揃える。f には、経過秒を渡す
fn render(sec: f32, mut f: impl FnMut(f32) -> (f32, f32)) -> StaticSoundData {
    let n = (sec * RATE as f32) as usize;
    let mut frames: Vec<Frame> = (0..n)
        .map(|i| {
            let (l, r) = f(i as f32 / RATE as f32);
            Frame { left: l, right: r }
        })
        .collect();
    let peak = frames.iter().map(|f| f.left.abs().max(f.right.abs())).fold(1e-6, f32::max);
    // 終わりは、ぷつっと切れないように 10 ミリ秒で消す
    let tail = (RATE as usize / 100).min(n);
    for (i, fr) in frames.iter_mut().enumerate() {
        let g = 0.9 / peak * ((n - i) as f32 / tail as f32).min(1.0);
        fr.left *= g;
        fr.right *= g;
    }
    StaticSoundData { sample_rate: RATE, frames: Arc::from(frames), settings: StaticSoundSettings::default(), slice: None }
}

/// 世界を渡る音。3 つを重ねる: 頭の鋭い電子音 / 歪ませた低い音 / 高さが動く風。
/// down = 落ちる(全部が下がっていく)。そうでなければ、浮かぶ(上がっていき、最後に開ける)
fn sweep(sec: f32, down: bool) -> StaticSoundData {
    let (mut nl, mut nr) = (Noise(0x2545_f491), Noise(0x9e37_79b9));
    let (mut fl, mut fr) = (Svf::default(), Svf::default());
    let (mut sub, mut zap, mut fm) = (0.0f32, 0.0f32, 0.0f32);
    render(sec, move |t| {
        let u = t / sec;
        let dt = 1.0 / RATE as f32;
        // 低い音: 歪ませて、小さなスピーカーでも聞こえる倍音を足す
        let sub_hz = if down { glide(170.0, 30.0, u) } else { glide(42.0, 260.0, u * u) };
        sub += sub_hz * dt;
        let sub_env = (t / 0.02).min(1.0) * if down { (1.0 - u).powf(0.8) } else { (u * 1.4).min(1.0) * (1.0 - u).powf(0.4) };
        let low = ((sub * TAU).sin() * 2.6).tanh() * sub_env * 0.8;
        // 頭の電子音: 高さが一気に動く。別の波で揺らして、金属っぽくする
        let z = (t / 0.28).min(1.0);
        let zap_hz = if down { glide(3200.0, 180.0, z) } else { glide(260.0, 3600.0, z) };
        fm += zap_hz * 2.71 * dt;
        zap += (zap_hz + (fm * TAU).sin() * zap_hz * 0.9) * dt;
        let zap_s = (zap * TAU).sin() * (1.0 - z).powi(2) * (t / 0.003).min(1.0) * 0.6;
        // 風: 癖の強いフィルターを通した雑音。左右を少し変えて、広がりを出す
        let wind_hz = if down { glide(7000.0, 160.0, u) } else { glide(220.0, 9000.0, u * u) };
        let wind_env = (t / 0.05).min(1.0) * if down { (1.0 - u).powf(1.2) } else { (0.25 + u * 0.75) * (1.0 - u).powf(0.3) };
        let wl = fl.run(nl.next(), wind_hz, 5.0).1 * wind_env * 1.3;
        let wr = fr.run(nr.next(), wind_hz * 1.12, 5.0).1 * wind_env * 1.3;
        (low + zap_s + wl, low + zap_s + wr)
    })
}

/// 次の世界に着いた音: 短い打撃 + 歪ませた低い衝撃 + 低い雑音の余韻
fn thump() -> StaticSoundData {
    let (mut nl, mut nr) = (Noise(0x1234_5677), Noise(0x0bad_cafe));
    let (mut fl, mut fr) = (Svf::default(), Svf::default());
    let mut phase = 0.0f32;
    render(1.4, move |t| {
        phase += (34.0 + 110.0 * (-t * 26.0).exp()) / RATE as f32;
        let boom = ((phase * TAU).sin() * 3.2).tanh() * (-t * 3.4).exp() * (t / 0.002).min(1.0);
        let click = (-t * 420.0).exp() * 0.7;
        let rumble = (-t * 5.0).exp() * 0.5;
        let (l, r) = (nl.next(), nr.next());
        (boom + l * click + fl.run(l, 420.0, 0.8).0 * rumble, boom + r * click + fr.run(r, 420.0, 0.8).0 * rumble)
    })
}

/// ブリンク: 短く鋭い。頭の打撃 → 高さが一気に落ちる電子音 → 空気が抜ける音。0.3 秒
fn blink() -> StaticSoundData {
    let (mut nl, mut nr) = (Noise(0x51ed_270b), Noise(0x7f4a_7c15));
    let (mut fl, mut fr) = (Svf::default(), Svf::default());
    let (mut ph, mut fm, mut body) = (0.0f32, 0.0f32, 0.0f32);
    render(0.32, move |t| {
        let dt = 1.0 / RATE as f32;
        // 胴: 低く短い音で、押し出される感じを足す
        body += glide(190.0, 70.0, t / 0.1) * dt;
        let thud = ((body * TAU).sin() * 2.0).tanh() * (-t * 24.0).exp() * (t / 0.002).min(1.0) * 0.55;
        let z = (t / 0.11).min(1.0);
        let hz = glide(3400.0, 380.0, z);
        fm += hz * 1.5 * dt;
        ph += (hz + (fm * TAU).sin() * hz * 0.6) * dt;
        let chirp = (ph * TAU).sin() * (1.0 - z).powf(1.5) * (t / 0.002).min(1.0) * 0.55;
        // 空気: 高い所から落ちてくる雑音。少し遅れて膨らみ、すぐ消える
        let air_hz = glide(11000.0, 1400.0, t / 0.26);
        let air_env = (t / 0.012).min(1.0) * (-t * 13.0).exp();
        let (l, r) = (nl.next(), nr.next());
        let snap = (-t * 900.0).exp() * 0.5;
        (thud + chirp + l * snap + fl.run(l, air_hz, 2.2).1 * air_env * 1.5, thud + chirp + r * snap + fr.run(r, air_hz * 0.9, 2.2).1 * air_env * 1.5)
    })
}

/// 一歩: 短く乾いた打音 + 低い胴。0.12 秒
fn step() -> StaticSoundData {
    let (mut nl, mut nr) = (Noise(0x3c6e_f372), Noise(0x1f83_d9ab));
    let (mut fl, mut fr) = (Svf::default(), Svf::default());
    let mut phase = 0.0f32;
    render(0.12, move |t| {
        phase += glide(120.0, 55.0, t / 0.06) / RATE as f32;
        let thud = (phase * TAU).sin() * (-t * 45.0).exp() * (t / 0.002).min(1.0) * 0.9;
        let tap = (-t * 90.0).exp() * 0.6;
        (thud + fl.run(nl.next(), 1800.0, 1.2).1 * tap, thud + fr.run(nr.next(), 2100.0, 1.2).1 * tap)
    })
}

/// 体の着地: こもった雑音の鈍い衝撃 + 足もとが擦れる音。高さのある音は入れない(入れると太鼓に聞こえる。2026-11-22 本人)。0.3 秒
fn landing() -> StaticSoundData {
    let (mut nl, mut nr) = (Noise(0x6a09_e667), Noise(0xbb67_ae85));
    let (mut fl, mut fr, mut sl, mut sr) = (Svf::default(), Svf::default(), Svf::default(), Svf::default());
    render(0.3, move |t| {
        let (l, r) = (nl.next(), nr.next());
        let hit = (-t * 38.0).exp() * (t / 0.003).min(1.0) * 3.0;
        let scuff = (-t * 16.0).exp() * (t / 0.02).min(1.0) * 0.25;
        (fl.run(l, 160.0, 0.7).0 * hit + sl.run(l, 2600.0, 0.9).1 * scuff, fr.run(r, 175.0, 0.7).0 * hit + sr.run(r, 2900.0, 0.9).1 * scuff)
    })
}

/// 落ちている間の風: 継ぎ目なく繰り返せる雑音。2 秒。終わりの先を頭に重ねて、つなぎ目を消す
fn wind() -> StaticSoundData {
    let (n, over) = (RATE as usize * 2, RATE as usize / 4);
    let (mut nl, mut nr) = (Noise(0x5be0_cd19), Noise(0x428a_2f98));
    let (mut fl, mut fr) = (Svf::default(), Svf::default());
    let raw: Vec<(f32, f32)> = (0..n + over).map(|_| (fl.run(nl.next(), 900.0, 1.4).1, fr.run(nr.next(), 1050.0, 1.4).1)).collect();
    let frames: Vec<Frame> = (0..n)
        .map(|i| {
            let u = (i as f32 / over as f32).min(1.0);
            let tail = raw[(n + i).min(n + over - 1)];
            Frame { left: (raw[i].0 * u + tail.0 * (1.0 - u)) * 0.5, right: (raw[i].1 * u + tail.1 * (1.0 - u)) * 0.5 }
        })
        .collect();
    StaticSoundData { sample_rate: RATE, frames: Arc::from(frames), settings: StaticSoundSettings::default(), slice: None }
}

fn write_wav(path: &Path, d: &StaticSoundData) -> std::io::Result<()> {
    let bytes = (d.frames.len() * 8) as u32;
    let mut out = Vec::with_capacity(bytes as usize + 44);
    out.extend(b"RIFF");
    out.extend((36 + bytes).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(3u16.to_le_bytes()); // 小数の形式
    out.extend(2u16.to_le_bytes());
    out.extend(RATE.to_le_bytes());
    out.extend((RATE * 8).to_le_bytes());
    out.extend(8u16.to_le_bytes());
    out.extend(32u16.to_le_bytes());
    out.extend(b"data");
    out.extend(bytes.to_le_bytes());
    for f in d.frames.iter() {
        out.extend(f.left.to_le_bytes());
        out.extend(f.right.to_le_bytes());
    }
    std::fs::write(path, out)
}
