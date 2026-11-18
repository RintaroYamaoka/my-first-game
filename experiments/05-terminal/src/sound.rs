//! 音(実験 6)。音のファイルは使わず、コードで作る。
//!
//! 声(Voice)は、起動時に決まった数だけ鳴らしっぱなしにしておく(普段は無音)。
//! ゲームの側は、毎コマ「高さ・大きさ・場所」を書き込むだけ。音を作る側(別のスレッド)が、それを読んで滑らかに追いかける。
//! 鳴らすたびに音を新しく用意しないので、出だしが遅れない。形の動きに、音を 1 コマ単位で合わせられる。
use bevy::audio::{AddAudioSource, ChannelCount, SampleRate, Source, SpatialScale};
use bevy::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

/// 同時に鳴らせる声の数
pub const VOICES: usize = 6;
const RATE: u32 = 44_100;
/// 世界の距離を、音の距離に直す割合(世界は広いので、そのままだと遠すぎて聞こえない)
const SCALE: f32 = 0.02;
/// 左右の耳の間(世界の単位)
pub const EARS: f32 = 10.0;

/// ゲームの側が書き、音を作る側が読む値(f32 を、そのままのビット列で持つ)
#[derive(Default)]
struct Tone {
    freq: AtomicU32,
    amp: AtomicU32,
}

#[derive(Asset, TypePath)]
struct Voice(Arc<Tone>);

struct Synth {
    tone: Arc<Tone>,
    phase: f32,
    freq: f32,
    amp: f32,
    n: u32,
    want: (f32, f32),
}

impl Iterator for Synth {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        // 32 個に 1 回だけ、ゲームの側の値を読む。あとは、ぷつっと鳴らないように滑らかに追いかける
        if self.n % 32 == 0 {
            self.want = (f32::from_bits(self.tone.freq.load(Relaxed)), f32::from_bits(self.tone.amp.load(Relaxed)));
        }
        self.n = self.n.wrapping_add(1);
        self.freq += (self.want.0 - self.freq) * 0.004;
        self.amp += (self.want.1 - self.amp) * 0.002;
        self.phase = (self.phase + self.freq / RATE as f32).fract();
        let p = self.phase * std::f32::consts::TAU;
        Some((p.sin() + 0.35 * (2.0 * p).sin() + 0.12 * (3.0 * p).sin()) * self.amp * 0.6)
    }
}

impl Source for Synth {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        ChannelCount::new(1).unwrap()
    }
    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(RATE).unwrap()
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

impl Decodable for Voice {
    type Decoder = Synth;
    fn decoder(&self) -> Synth {
        Synth { tone: self.0.clone(), phase: 0.0, freq: 220.0, amp: 0.0, n: 0, want: (220.0, 0.0) }
    }
}

/// 声の一覧。`set` で、声ごとに「場所・高さ(Hz)・大きさ(0〜1)」を決める
#[derive(Resource)]
pub struct Voices {
    tones: Vec<(Arc<Tone>, Entity)>,
    /// 今コマ、何番まで使ったか
    used: usize,
}

impl Voices {
    /// 空いている声を 1 つ使って鳴らす(1 コマぶん。鳴らし続けるなら、毎コマ呼ぶ)
    pub fn sing(&mut self, tf: &mut Query<&mut Transform, With<Emitter>>, pos: Vec3, freq: f32, amp: f32) {
        let Some((tone, e)) = self.tones.get(self.used) else { return };
        self.used += 1;
        tone.freq.store(freq.to_bits(), Relaxed);
        tone.amp.store(amp.clamp(0.0, 1.0).to_bits(), Relaxed);
        if let Ok(mut t) = tf.get_mut(*e) {
            t.translation = pos;
        }
    }
}

#[derive(Component)]
pub struct Emitter;

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<Voice>().add_systems(Startup, setup).add_systems(Last, hush);
    }
}

fn setup(mut commands: Commands, mut assets: ResMut<Assets<Voice>>) {
    let tones = (0..VOICES)
        .map(|_| {
            let tone = Arc::new(Tone::default());
            let e = commands
                .spawn((AudioPlayer(assets.add(Voice(tone.clone()))), PlaybackSettings::LOOP.with_spatial(true).with_spatial_scale(SpatialScale::new(SCALE)), Transform::default(), Emitter))
                .id();
            (tone, e)
        })
        .collect();
    commands.insert_resource(Voices { tones, used: 0 });
}

/// コマの終わりに、今コマ使われなかった声を黙らせる
fn hush(mut voices: ResMut<Voices>) {
    let used = std::mem::take(&mut voices.used);
    for (tone, _) in voices.tones.iter().skip(used) {
        tone.amp.store(0f32.to_bits(), Relaxed);
    }
}
