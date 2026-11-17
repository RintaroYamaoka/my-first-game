//! 本物のターミナルを、世界の画面の上に重ねる。
//!   中身: 端末の中核部品(alacritty_terminal)が、本物のシェルを擬似端末(PTY)で動かす。
//!   表示: 文字のマス目を、Bevy の文字として自分で描く。
//!   入力: キーを、端末が受け取るバイト列に直して送る。
//! F1(または T)で開く。F1 で閉じる。

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::tty;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::text::LineBreak;
use bevy::window::{CursorGrabMode, CursorOptions};
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

const COLS: usize = 84;
const LINES: usize = 34;
const FONT: f32 = 16.0;
const CW: f32 = FONT * 0.6; // 既定の文字(FiraMono)は、幅が高さの 0.6 倍
const CH: f32 = 20.0;
const PAD: f32 = 14.0;
const BG: [f32; 3] = [0.045, 0.052, 0.068];
const FG: [f32; 3] = [0.86, 0.88, 0.9];
/// 基本の 16 色
const PALETTE: [[u8; 3]; 16] = [
    [0x1d, 0x1f, 0x21], [0xe5, 0x43, 0x2a], [0x8c, 0xc8, 0x5f], [0xe6, 0xc3, 0x5c], [0x5f, 0x9f, 0xe6], [0xc0, 0x7a, 0xd8], [0x56, 0xc2, 0xc6], [0xc5, 0xc8, 0xc6],
    [0x66, 0x6a, 0x70], [0xff, 0x6b, 0x55], [0xa6, 0xe2, 0x7a], [0xf5, 0xd9, 0x7a], [0x82, 0xb8, 0xff], [0xd9, 0x9a, 0xee], [0x7a, 0xdc, 0xe0], [0xff, 0xff, 0xff],
];

struct Size;
impl Dimensions for Size {
    fn total_lines(&self) -> usize { LINES }
    fn screen_lines(&self) -> usize { LINES }
    fn columns(&self) -> usize { COLS }
}

#[derive(Clone)]
struct Proxy(Sender<Event>);
impl EventListener for Proxy {
    fn send_event(&self, event: Event) {
        let _ = self.0.send(event);
    }
}

#[derive(Resource)]
pub struct Terminal {
    pub open: bool,
    term: Arc<FairMutex<Term<Proxy>>>,
    tx: EventLoopSender,
    rx: Mutex<Receiver<Event>>,
    alive: bool,
    dirty: bool,
    /// 行ごとの、前に描いた内容(変わった行だけ描き直す)
    shown: Vec<Vec<Run>>,
    /// キーを送った時刻(画面に反映されるまでの時間を測る)
    sent_at: Option<Instant>,
    last_ms: f32,
    worst_ms: f32,
    fps: f32,
}

/// 日本語の文字。実験の間は、パソコンに入っている文字のファイルを借りる(製品では、配ってよい文字を同梱する)
#[derive(Resource)]
struct Glyphs(FontSource);

fn japanese_font() -> Option<Vec<u8>> {
    let home = std::env::var("HOME").unwrap_or_default();
    let candidates = [
        std::env::var("FONT").unwrap_or_default(),
        "C:\\Windows\\Fonts\\BIZ-UDGothicR.ttc".into(),
        "/mnt/c/Windows/Fonts/BIZ-UDGothicR.ttc".into(),
        format!("{home}/.fonts/NotoSansCJK-VF.otf.ttc"),
    ];
    candidates.iter().filter(|p| !p.is_empty()).find_map(|p| std::fs::read(p).ok())
}

#[derive(Component)]
struct Panel;
#[derive(Component)]
struct Row(usize);
#[derive(Component)]
struct CursorBlock;
#[derive(Component)]
struct Status;

/// 同じ色が続く、ひとつながりの文字
#[derive(Clone, PartialEq)]
struct Run {
    col: usize,
    cells: usize,
    wide: bool,
    text: String,
    fg: [f32; 3],
    bg: Option<[f32; 3]>,
}

pub struct TerminalPlugin {
    pub cwd: PathBuf,
}

impl Plugin for TerminalPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(spawn_shell(&self.cwd))
            .add_systems(Startup, setup)
            .add_systems(Update, (keys, pump, render, auto_type).chain());
    }
}

/// 本物のシェルを、擬似端末で起動する
fn spawn_shell(cwd: &std::path::Path) -> Terminal {
    let mut options = tty::Options::default();
    options.env.insert("TERM".into(), "xterm-256color".into());
    #[cfg(windows)]
    {
        // Windows では、まだ同梱の Linux がない(実験 5)。それまでは WSL のシェルを借りる
        let cmd = std::env::var("SHELL_CMD").unwrap_or_else(|_| "wsl.exe".into());
        let args = if cmd == "wsl.exe" { vec!["--cd".to_string(), cwd.to_string_lossy().to_string()] } else { vec![] };
        options.shell = Some(tty::Shell::new(cmd, args));
    }
    #[cfg(not(windows))]
    {
        let cmd = std::env::var("SHELL_CMD").or_else(|_| std::env::var("SHELL")).unwrap_or_else(|_| "/bin/bash".into());
        options.shell = Some(tty::Shell::new(cmd, vec![]));
        options.working_directory = cwd.canonicalize().ok();
    }
    let size = WindowSize { num_lines: LINES as u16, num_cols: COLS as u16, cell_width: CW as u16, cell_height: CH as u16 };
    let (etx, erx) = channel();
    let proxy = Proxy(etx);
    let term = Arc::new(FairMutex::new(Term::new(Config::default(), &Size, proxy.clone())));
    let pty = tty::new(&options, size, 0).expect("シェルを起動できない");
    let event_loop = EventLoop::new(term.clone(), proxy, pty, false, false).expect("端末の読み書きを始められない");
    let tx = event_loop.channel();
    event_loop.spawn();
    Terminal {
        open: false, term, tx, rx: Mutex::new(erx), alive: true, dirty: true,
        shown: vec![vec![]; LINES], sent_at: None, last_ms: 0.0, worst_ms: 0.0, fps: 0.0,
    }
}

fn rgb(c: [f32; 3], a: f32) -> Color {
    Color::srgba(c[0], c[1], c[2], a)
}

fn setup(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
    let base = FontSource::Handle(Handle::default());
    commands.insert_resource(Glyphs(match japanese_font() {
        Some(bytes) => FontSource::List(vec![base, FontSource::Handle(fonts.add(Font::from_bytes(bytes)))]),
        None => base,
    }));
    let (w, h) = (COLS as f32 * CW, LINES as f32 * CH);
    commands
        .spawn((
            Node { position_type: PositionType::Absolute, right: px(22), top: px(22), width: px(w + PAD * 2.0), height: px(h + PAD * 2.0 + CH + 6.0), ..default() },
            BackgroundColor(rgb(BG, 0.94)),
            GlobalZIndex(50),
            Visibility::Hidden,
            Panel,
        ))
        .with_children(|p| {
            for line in 0..LINES {
                p.spawn((Node { position_type: PositionType::Absolute, left: px(PAD), top: px(PAD + line as f32 * CH), width: px(w), height: px(CH), ..default() }, Row(line)));
            }
            p.spawn((
                Node { position_type: PositionType::Absolute, width: px(CW), height: px(CH), ..default() },
                BackgroundColor(rgb(FG, 0.55)),
                CursorBlock,
            ));
            p.spawn((
                Text::new(""),
                TextFont { font_size: FontSize::Px(13.0), ..default() },
                TextColor(rgb(FG, 0.6)),
                Node { position_type: PositionType::Absolute, left: px(PAD), top: px(PAD + h + 8.0), ..default() },
                Status,
            ));
        });
}

/// キーを、端末が受け取るバイト列に直す
fn encode(ev: &KeyboardInput, ctrl: bool, alt: bool, app_cursor: bool) -> Option<Vec<u8>> {
    let arrow = |c: char| format!("\x1b{}{c}", if app_cursor { 'O' } else { '[' }).into_bytes();
    let mut out = match &ev.logical_key {
        Key::Enter => b"\r".to_vec(),
        Key::Backspace => vec![0x7f],
        Key::Tab => b"\t".to_vec(),
        Key::Escape => vec![0x1b],
        Key::Space if ctrl => vec![0],
        Key::ArrowUp => arrow('A'),
        Key::ArrowDown => arrow('B'),
        Key::ArrowRight => arrow('C'),
        Key::ArrowLeft => arrow('D'),
        Key::Home => b"\x1b[H".to_vec(),
        Key::End => b"\x1b[F".to_vec(),
        Key::Delete => b"\x1b[3~".to_vec(),
        Key::PageUp => b"\x1b[5~".to_vec(),
        Key::PageDown => b"\x1b[6~".to_vec(),
        Key::Character(s) if ctrl => {
            // Ctrl+C は 3、Ctrl+D は 4 …
            let c = s.chars().next()?.to_ascii_uppercase();
            if !('@'..='_').contains(&c) {
                return None;
            }
            vec![c as u8 & 0x1f]
        }
        _ => ev.text.as_ref()?.as_bytes().to_vec(),
    };
    if alt {
        out.insert(0, 0x1b);
    }
    Some(out)
}

fn keys(
    mut events: MessageReader<KeyboardInput>,
    held: Res<ButtonInput<KeyCode>>,
    mut t: ResMut<Terminal>,
    mut cursor: Single<&mut CursorOptions>,
) {
    let ctrl = held.pressed(KeyCode::ControlLeft) || held.pressed(KeyCode::ControlRight);
    let alt = held.pressed(KeyCode::AltLeft);
    let shift = held.pressed(KeyCode::ShiftLeft) || held.pressed(KeyCode::ShiftRight);
    for ev in events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        let toggle = ev.key_code == KeyCode::F1 || (!t.open && ev.key_code == KeyCode::KeyT);
        if toggle && !ev.repeat {
            t.open = !t.open;
            t.dirty = true;
            if t.open {
                // 開いている間は、マウスを放す
                cursor.grab_mode = CursorGrabMode::None;
                cursor.visible = true;
            }
            continue;
        }
        if !t.open || !t.alive {
            continue;
        }
        // Shift + PageUp / PageDown: 過去の出力へ戻る
        if shift && matches!(ev.logical_key, Key::PageUp | Key::PageDown) {
            t.term.lock().scroll_display(if ev.logical_key == Key::PageUp { Scroll::PageUp } else { Scroll::PageDown });
            t.dirty = true;
            continue;
        }
        let app_cursor = t.term.lock().mode().contains(TermMode::APP_CURSOR);
        if let Some(bytes) = encode(ev, ctrl, alt, app_cursor) {
            t.term.lock().scroll_display(Scroll::Bottom);
            let _ = t.tx.send(Msg::Input(Cow::Owned(bytes)));
            t.sent_at.get_or_insert_with(Instant::now);
        }
    }
}

/// 端末からの知らせを受け取る
fn pump(mut t: ResMut<Terminal>) {
    let events: Vec<Event> = t.rx.lock().unwrap().try_iter().collect();
    for ev in events {
        match ev {
            Event::Wakeup => t.dirty = true,
            // 端末への問い合わせの返事は、シェルへ送り返す
            Event::PtyWrite(s) => {
                let _ = t.tx.send(Msg::Input(Cow::Owned(s.into_bytes())));
            }
            Event::Exit | Event::ChildExit(_) => {
                t.alive = false;
                t.open = false;
                t.dirty = true;
            }
            _ => {}
        }
    }
}

fn color(c: AnsiColor, default: [f32; 3]) -> [f32; 3] {
    let from = |v: [u8; 3]| [v[0] as f32 / 255.0, v[1] as f32 / 255.0, v[2] as f32 / 255.0];
    let index = |i: usize| match i {
        0..=15 => from(PALETTE[i]),
        16..=231 => {
            let level = |v: usize| if v == 0 { 0 } else { (55 + v * 40) as u8 };
            let n = i - 16;
            from([level(n / 36), level(n / 6 % 6), level(n % 6)])
        }
        232..=255 => from([(8 + (i - 232) * 10) as u8; 3]),
        _ => default,
    };
    match c {
        AnsiColor::Spec(v) => from([v.r, v.g, v.b]),
        AnsiColor::Indexed(i) => index(i as usize),
        AnsiColor::Named(NamedColor::Foreground) => FG,
        AnsiColor::Named(NamedColor::Background) => BG,
        AnsiColor::Named(n) => index(n as usize),
    }
}

/// 端末のマス目を、行ごとの「同じ色の続き」にまとめる
fn runs(t: &Terminal) -> (Vec<Vec<Run>>, Option<(usize, usize)>) {
    let term = t.term.lock();
    let content = term.renderable_content();
    let offset = content.display_offset as i32;
    let mut rows: Vec<Vec<Run>> = vec![vec![]; LINES];
    for cell in content.display_iter {
        let line = cell.point.line.0 + offset;
        if line < 0 || line as usize >= LINES || cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
            continue;
        }
        let (mut fg, mut bg) = (color(cell.fg, FG), color(cell.bg, BG));
        if cell.flags.contains(Flags::INVERSE) {
            std::mem::swap(&mut fg, &mut bg);
        }
        if cell.flags.contains(Flags::DIM) {
            fg = fg.map(|v| v * 0.6);
        }
        let bg = (bg != BG).then_some(bg);
        let ch = if cell.flags.contains(Flags::HIDDEN) { ' ' } else { cell.c };
        if ch == ' ' && bg.is_none() {
            continue;
        }
        let wide = if cell.flags.contains(Flags::WIDE_CHAR) { 2 } else { 1 };
        let col = cell.point.column.0;
        let row = &mut rows[line as usize];
        match row.last_mut() {
            Some(r) if wide == 1 && !r.wide && r.col + r.cells == col && r.fg == fg && r.bg == bg => {
                r.text.push(ch);
                r.cells += wide;
            }
            // 全角の文字は、一文字ずつ置く(マス目からずれないように)
            _ => row.push(Run { col, cells: wide, wide: wide == 2, text: ch.to_string(), fg, bg }),
        }
    }
    let p = content.cursor.point;
    let line = p.line.0 + offset;
    let cursor = (line >= 0 && (line as usize) < LINES).then(|| (line as usize, p.column.0));
    (rows, cursor)
}

fn render(
    mut commands: Commands,
    time: Res<Time>,
    mut t: ResMut<Terminal>,
    mut panel: Single<&mut Visibility, With<Panel>>,
    rows: Query<(Entity, &Row)>,
    mut cursor: Single<&mut Node, With<CursorBlock>>,
    mut status: Single<&mut Text, With<Status>>,
    glyphs: Res<Glyphs>,
) {
    t.fps += (1.0 / time.delta_secs().max(1e-4) - t.fps) * 0.05;
    **panel = if t.open { Visibility::Visible } else { Visibility::Hidden };
    if !t.open {
        return;
    }
    status.0 = format!(
        "F1 close   key -> screen {:.1} ms (worst {:.1})   {:.0} fps{}",
        t.last_ms, t.worst_ms, t.fps, if t.alive { "" } else { "   [shell exited]" }
    );
    if !t.dirty {
        return;
    }
    t.dirty = false;
    let (new, at) = runs(&t);
    for (entity, row) in &rows {
        if t.shown[row.0] == new[row.0] {
            continue;
        }
        commands.entity(entity).despawn_children();
        commands.entity(entity).with_children(|p| {
            for r in &new[row.0] {
                p.spawn((
                    Text::new(r.text.clone()),
                    TextFont { font: glyphs.0.clone(), font_size: FontSize::Px(FONT), ..default() },
                    TextColor(rgb(r.fg, 1.0)),
                    TextLayout { linebreak: LineBreak::NoWrap, ..default() },
                    Node { position_type: PositionType::Absolute, left: px(r.col as f32 * CW), top: px(0), width: px(r.cells as f32 * CW), height: px(CH), ..default() },
                    BackgroundColor(r.bg.map_or(Color::NONE, |c| rgb(c, 1.0))),
                ));
            }
        });
    }
    t.shown = new;
    match at {
        Some((line, col)) => {
            cursor.left = px(PAD + col as f32 * CW);
            cursor.top = px(PAD + line as f32 * CH);
            cursor.display = Display::Flex;
        }
        None => cursor.display = Display::None,
    }
    // キーを送ってから、その結果を画面に描くまでの時間
    if let Some(sent) = t.sent_at.take() {
        t.last_ms = sent.elapsed().as_secs_f32() * 1000.0;
        t.worst_ms = t.worst_ms.max(t.last_ms);
    }
}

/// TYPE が指定されていたら、開いて、その文字を打つ(確かめるため)。"\n" は Enter
fn auto_type(time: Res<Time>, mut t: ResMut<Terminal>, mut done: Local<bool>) {
    if *done || time.elapsed_secs() < 3.5 {
        return;
    }
    *done = true;
    if let Ok(text) = std::env::var("TYPE") {
        t.open = true;
        t.dirty = true;
        let _ = t.tx.send(Msg::Input(Cow::Owned(text.replace("\\n", "\r").into_bytes())));
        t.sent_at = Some(Instant::now());
    }
}
