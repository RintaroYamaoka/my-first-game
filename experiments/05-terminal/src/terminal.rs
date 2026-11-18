//! 本物のターミナルを、世界の画面の上に重ねる。
//!   中身: 端末の中核部品(alacritty_terminal)が、本物のシェルを擬似端末(PTY)で動かす。
//!   表示: 文字のマス目を、Bevy の文字として自分で描く。
//!   入力: キーを、端末が受け取るバイト列に直して送る。
//! F1(または T)で開く。F1 で閉じる。

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::tty;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor, Processor};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::clipboard::Clipboard;
use bevy::prelude::*;
use bevy::text::LineBreak;
use bevy::window::{CursorGrabMode, CursorOptions};
use std::borrow::Cow;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
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
const SELECTED: [f32; 3] = [0.22, 0.33, 0.52];
const MARGIN: f32 = 22.0;
/// パネルが画面の右から占める幅(世界の側が、空いている場所を知るため)
pub const PANEL_W: f32 = COLS as f32 * CW + PAD * 2.0 + MARGIN;
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
    /// キー入力の行き先がターミナルか(開いていても、外をクリックすると世界の操作に戻る)
    pub focus: bool,
    /// マウスが、ターミナルの上にあるか(つかんでいないとき)
    pub hover: bool,
    term: Arc<FairMutex<Term<Proxy>>>,
    link: Link,
    /// 同梱の Linux の中のファイルの写し(この PC の上)と、変化の回数。世界は、これを見て作り直す
    pub mirror: Option<(PathBuf, Arc<AtomicU64>)>,
    rx: Mutex<Receiver<Event>>,
    alive: bool,
    dirty: bool,
    /// 行ごとの、前に描いた内容(変わった行だけ描き直す)
    shown: Vec<Vec<Run>>,
    /// マウスで選んでいる途中か
    dragging: bool,
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

/// ターミナルの処理のまとまり。世界の操作は、この後に動かす(同じクリックを、両方が受け取らないように)
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerminalSystems;

pub struct TerminalPlugin {
    pub cwd: PathBuf,
}

impl Plugin for TerminalPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(spawn_shell(&self.cwd))
            .add_systems(Startup, setup)
            .add_systems(Update, (keys, mouse, pump, render, auto_type).chain().in_set(TerminalSystems));
    }
}

/// シェルへの通り道
enum Link {
    /// 擬似端末(この PC のシェルを、そのまま動かすとき)
    Pty(EventLoopSender),
    /// TCP の接続(同梱の Linux。QEMU の直列の線=シリアルと、じかにつながる)
    Qemu(TcpStream, Child),
}

impl Link {
    fn send(&mut self, bytes: Vec<u8>) {
        match self {
            Link::Pty(tx) => {
                let _ = tx.send(Msg::Input(Cow::Owned(bytes)));
            }
            Link::Qemu(stream, _) => {
                let _ = stream.write_all(&bytes);
            }
        }
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        if let Link::Qemu(_, child) = self {
            let _ = child.kill();
        }
    }
}

/// `--linux <イメージのフォルダ>` が指定されていたら、QEMU を動かすコマンドを返す。
///   `--accel <tcg|whpx|kvm>`(既定は tcg = どの PC でも動く、遅い方式)、`--qemu <QEMU の場所>`
/// QEMU の直列の線は、ゲームが待ち受ける TCP の口(port)へつなぐ。
///   画面入力(標準入力)経由にしない理由: Windows 版の QEMU は、まとめて届いた文字を 8 文字で取りこぼす(2026-11-18 に確認)
fn linux_command(port: u16, watch_port: u16) -> Option<Command> {
    let args: Vec<String> = std::env::args().collect();
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned());
    let image = std::path::PathBuf::from(opt("--linux")?);
    let accel = opt("--accel").unwrap_or_else(|| "tcg".into());
    let file = |name: &str| image.join(name).to_string_lossy().to_string();
    let mut qemu: Vec<String> = Vec::new();
    let mut add = |items: &[&str]| qemu.extend(items.iter().map(|s| s.to_string()));
    match accel.as_str() {
        "tcg" => add(&["-accel", "tcg,thread=multi", "-cpu", "max"]),
        "kvm" => add(&["-accel", "kvm", "-cpu", "host"]),
        other => add(&["-accel", other]),
    }
    let append = format!("console=ttyS0 quiet rows={LINES} cols={COLS}");
    add(&["-smp", "2", "-m", "1024", "-append", &append, "-display", "none", "-monitor", "none", "-no-reboot"]);
    // 1 本目: ターミナル。2 本目: ファイルの変化の知らせ
    add(&["-serial", &format!("tcp:127.0.0.1:{port},nodelay=on"), "-serial", &format!("tcp:127.0.0.1:{watch_port},nodelay=on")]);
    let default_qemu = if cfg!(windows) { "C:\\Program Files\\qemu\\qemu-system-x86_64.exe" } else { "qemu-system-x86_64" };
    let program = opt("--qemu").unwrap_or_else(|| default_qemu.into());
    let mut cmd;
    if cfg!(windows) || Command::new(&program).arg("--version").output().is_ok() {
        add(&["-kernel", &file("vmlinuz"), "-initrd", &file("initramfs.gz")]);
        cmd = Command::new(program);
        cmd.args(qemu);
    } else {
        // 開発用: この PC に QEMU がなければ、測定用の Docker イメージの中の QEMU を使う(イメージも、その中のもの)
        cmd = Command::new("docker");
        cmd.args(["run", "--rm", "--network", "host"]);
        if accel == "kvm" {
            cmd.args(["--device", "/dev/kvm"]);
        }
        cmd.args(["my-first-game-linux:bench", "qemu-system-x86_64"]).args(qemu);
        cmd.args(["-kernel", "/out/vmlinuz", "-initrd", "/out/initramfs.gz"]);
    }
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    {
        // QEMU の黒い窓を出さない
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    Some(cmd)
}

/// 同梱の Linux を起動して、つなぐ。QEMU からの文字は、別のスレッドで読んで、端末のマス目に反映する
fn spawn_linux(term: &Arc<FairMutex<Term<Proxy>>>, proxy: &Proxy) -> Option<(Link, (PathBuf, Arc<AtomicU64>))> {
    let listener = TcpListener::bind("127.0.0.1:0").expect("ゲームと Linux の通り道を作れない");
    let watch = TcpListener::bind("127.0.0.1:0").expect("ゲームと Linux の通り道を作れない");
    let mut cmd = linux_command(listener.local_addr().unwrap().port(), watch.local_addr().unwrap().port())?;
    let child = cmd.spawn().expect("QEMU を起動できない");
    let (stream, _) = listener.accept().expect("QEMU がつながらない");
    let (watch_stream, _) = watch.accept().expect("QEMU がつながらない");
    let mirror = std::env::temp_dir().join(format!("my-first-game-{}", std::process::id())).join("sample");
    let _ = std::fs::remove_dir_all(&mirror);
    std::fs::create_dir_all(&mirror).expect("写しを置く場所を作れない");
    let changes = Arc::new(AtomicU64::new(0));
    {
        let (mirror, changes) = (mirror.clone(), changes.clone());
        std::thread::spawn(move || watch_files(watch_stream, &mirror, &changes));
    }
    let _ = stream.set_nodelay(true);
    let (mut reader, term, proxy) = (stream.try_clone().unwrap(), term.clone(), proxy.clone());
    std::thread::spawn(move || {
        let mut parser: Processor = Processor::new();
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(n) if n > 0 => {
                    parser.advance(&mut *term.lock(), &buf[..n]);
                    proxy.send_event(Event::Wakeup);
                }
                _ => break,
            }
        }
        proxy.send_event(Event::Exit);
    });
    Some((Link::Qemu(stream, child), (mirror, changes)))
}

/// 見張り番(Linux の中の agent.sh)からの知らせを読んで、この PC の上の写しに反映する
fn watch_files(stream: TcpStream, mirror: &std::path::Path, changes: &AtomicU64) {
    use std::io::BufRead;
    // 写しの外へ出る道筋は受け付けない
    let safe = |rel: &str| (!rel.is_empty() && !rel.starts_with('/') && !rel.split('/').any(|p| p == ".." || p.is_empty())).then(|| mirror.join(rel));
    for line in std::io::BufReader::new(stream).lines() {
        let Ok(line) = line else { break };
        let line = line.trim_end_matches('\r');
        if line == "S" {
            changes.fetch_add(1, Ordering::Relaxed);
        } else if let Some(rest) = line.strip_prefix("F ") {
            let Some((b64, rel)) = rest.split_once(' ') else { continue };
            let Some(path) = safe(rel) else { continue };
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(path, if b64 == "-" { vec![] } else { base64(b64) });
        } else if let Some(rel) = line.strip_prefix("D ") {
            let Some(path) = safe(rel) else { continue };
            if path.is_dir() {
                let _ = std::fs::remove_dir_all(path);
            } else {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

fn base64(text: &str) -> Vec<u8> {
    let (mut out, mut acc, mut bits) = (Vec::with_capacity(text.len() * 3 / 4), 0u32, 0);
    for c in text.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        };
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

/// 本物のシェルを、擬似端末で起動する
fn spawn_shell(cwd: &std::path::Path) -> Terminal {
    let mut options = tty::Options::default();
    options.env.insert("TERM".into(), "xterm-256color".into());
    let size = WindowSize { num_lines: LINES as u16, num_cols: COLS as u16, cell_width: CW as u16, cell_height: CH as u16 };
    let (etx, erx) = channel();
    let proxy = Proxy(etx);
    let term = Arc::new(FairMutex::new(Term::new(Config::default(), &Size, proxy.clone())));
    let linux = spawn_linux(&term, &proxy);
    let (linux_link, mirror) = match linux {
        Some((l, m)) => (Some(l), Some(m)),
        None => (None, None),
    };
    let link = linux_link.unwrap_or_else(|| {
        #[cfg(windows)]
        {
            // 同梱の Linux を指定しないときは、WSL のシェルを借りる
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
        let pty = tty::new(&options, size, 0).expect("シェルを起動できない");
        let event_loop = EventLoop::new(term.clone(), proxy, pty, false, false).expect("端末の読み書きを始められない");
        let tx = event_loop.channel();
        event_loop.spawn();
        Link::Pty(tx)
    });
    Terminal {
        open: false, focus: false, hover: false, term, link, mirror, rx: Mutex::new(erx), alive: true, dirty: true,
        shown: vec![vec![]; LINES], dragging: false, sent_at: None, last_ms: 0.0, worst_ms: 0.0, fps: 0.0,
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
            Node { position_type: PositionType::Absolute, right: px(MARGIN), top: px(MARGIN), width: px(w + PAD * 2.0), height: px(h + PAD * 2.0 + CH + 6.0), ..default() },
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
        // F1: 開く・閉じる。T: 開く、または(開いていて世界を操作中なら)ターミナルへ戻る
        let back = ev.key_code == KeyCode::KeyT && !(t.open && t.focus);
        if (ev.key_code == KeyCode::F1 || back) && !ev.repeat {
            t.open = back || !t.open;
            t.focus = t.open;
            t.dirty = true;
            if t.open {
                // 開いている間は、マウスを放す
                cursor.grab_mode = CursorGrabMode::None;
                cursor.visible = true;
            }
            continue;
        }
        if !t.open || !t.focus || !t.alive {
            continue;
        }
        // Shift + PageUp / PageDown: 過去の出力へ戻る
        if shift && matches!(ev.logical_key, Key::PageUp | Key::PageDown) {
            t.term.lock().scroll_display(if ev.logical_key == Key::PageUp { Scroll::PageUp } else { Scroll::PageDown });
            t.dirty = true;
            continue;
        }
        // Ctrl+Shift+C / V は、コピーと貼り付け(mouse で扱う)。Linux へは送らない
        if ctrl && shift && matches!(ev.key_code, KeyCode::KeyC | KeyCode::KeyV) {
            continue;
        }
        let app_cursor = t.term.lock().mode().contains(TermMode::APP_CURSOR);
        if let Some(bytes) = encode(ev, ctrl, alt, app_cursor) {
            t.term.lock().scroll_display(Scroll::Bottom);
            t.link.send(bytes);
            t.sent_at.get_or_insert_with(Instant::now);
        }
    }
}

/// マウスで文字を選ぶ。選び終えると、そのままコピーされる。右クリック(または Ctrl+Shift+V)で貼り付ける
fn mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    held: Res<ButtonInput<KeyCode>>,
    window: Single<&Window>,
    mut grab: Single<&mut CursorOptions>,
    mut clipboard: ResMut<Clipboard>,
    mut t: ResMut<Terminal>,
) {
    // マウスが、ターミナルの上にあるか(世界の操作でマウスをつかんでいる間は、数えない)
    let free = grab.grab_mode == CursorGrabMode::None;
    let (w, h) = (COLS as f32 * CW + PAD * 2.0, LINES as f32 * CH + PAD * 2.0 + CH + 6.0);
    t.hover = t.open && free && window.cursor_position().is_some_and(|c| {
        let x = c.x - (window.width() - MARGIN - w);
        x >= 0.0 && x <= w && c.y >= MARGIN && c.y <= MARGIN + h
    });
    if buttons.just_pressed(MouseButton::Left) && t.open && free && t.hover != t.focus {
        // 上をクリック: ターミナルへ。外をクリック: 世界の操作へ(ターミナルは開いたまま)
        t.focus = t.hover;
        t.dragging = false;
        t.dirty = true;
        if t.focus {
            grab.visible = true;
        }
        return;
    }
    if !t.open || !t.focus || !t.alive {
        t.dragging = false;
        return;
    }
    let combo = (held.pressed(KeyCode::ControlLeft) || held.pressed(KeyCode::ControlRight)) && (held.pressed(KeyCode::ShiftLeft) || held.pressed(KeyCode::ShiftRight));
    // カーソルの下の、マス目の位置
    let cell = window.cursor_position().map(|c| {
        let x = (c.x - (window.width() - MARGIN - PAD - COLS as f32 * CW)) / CW;
        let y = (c.y - MARGIN - PAD) / CH;
        let inside = x >= 0.0 && y >= 0.0 && x < COLS as f32 && y < LINES as f32;
        let (col, line) = (x.clamp(0.0, COLS as f32 - 1.0), y.clamp(0.0, LINES as f32 - 1.0));
        let offset = t.term.lock().grid().display_offset() as i32;
        let side = if col.fract() < 0.5 { Side::Left } else { Side::Right };
        (Point::new(Line(line as i32 - offset), Column(col as usize)), side, inside)
    });
    if let Some((point, side, inside)) = cell {
        if buttons.just_pressed(MouseButton::Left) {
            t.term.lock().selection = inside.then(|| Selection::new(SelectionType::Simple, point, side));
            t.dragging = inside;
            t.dirty = true;
        } else if t.dragging && buttons.pressed(MouseButton::Left) {
            if let Some(sel) = t.term.lock().selection.as_mut() {
                sel.update(point, side);
            }
            t.dirty = true;
        }
    }
    let copy = (t.dragging && buttons.just_released(MouseButton::Left)) || (combo && held.just_pressed(KeyCode::KeyC));
    if buttons.just_released(MouseButton::Left) {
        t.dragging = false;
    }
    if copy {
        if let Some(text) = t.term.lock().selection_to_string().filter(|s| !s.is_empty()) {
            let _ = clipboard.set_text(text);
        }
    }
    if buttons.just_pressed(MouseButton::Right) || (combo && held.just_pressed(KeyCode::KeyV)) {
        paste(&mut t, &mut clipboard);
    }
}

fn paste(t: &mut Terminal, clipboard: &mut Clipboard) {
    let got = clipboard.fetch_text().poll_result();
    let Some(Ok(text)) = got else {
        eprintln!("paste: クリップボードを読めない: {got:?}");
        return;
    };
    let text = text.replace("\r\n", "\r").replace('\n', "\r");
    // 貼り付けだと分かる印で囲む(対応しているプログラムは、途中の改行で実行しない)
    // Windows の擬似端末(ConPTY)経由のときは、印を付けない: 印ごと貼り付けが捨てられる(2026-11-18 に確認)
    let lossy = cfg!(windows) && matches!(t.link, Link::Pty(_));
    let bracketed = !lossy && t.term.lock().mode().contains(TermMode::BRACKETED_PASTE);
    eprintln!("paste: {} 文字 (bracketed {bracketed})", text.chars().count());
    let bytes = if bracketed { format!("\x1b[200~{text}\x1b[201~") } else { text }.into_bytes();
    t.term.lock().scroll_display(Scroll::Bottom);
    t.link.send(bytes);
}

/// 端末からの知らせを受け取る
fn pump(mut t: ResMut<Terminal>) {
    let events: Vec<Event> = t.rx.lock().unwrap().try_iter().collect();
    for ev in events {
        match ev {
            Event::Wakeup => t.dirty = true,
            // 端末への問い合わせの返事は、シェルへ送り返す
            Event::PtyWrite(s) => {
                t.link.send(s.into_bytes());
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
    let selection = content.selection;
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
        let bg = if selection.is_some_and(|s| s.contains(cell.point)) { Some(SELECTED) } else { (bg != BG).then_some(bg) };
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
    mut panel: Single<(&mut Visibility, &mut BackgroundColor), With<Panel>>,
    rows: Query<(Entity, &Row)>,
    mut cursor: Single<&mut Node, With<CursorBlock>>,
    mut status: Single<&mut Text, With<Status>>,
    glyphs: Res<Glyphs>,
) {
    t.fps += (1.0 / time.delta_secs().max(1e-4) - t.fps) * 0.05;
    *panel.0 = if t.open { Visibility::Visible } else { Visibility::Hidden };
    // 世界を操作している間は、ターミナルを薄くして、後ろの世界を見やすくする
    panel.1.0 = rgb(BG, if t.focus { 0.94 } else { 0.45 });
    if !t.open {
        return;
    }
    status.0 = if t.focus {
        format!(
            "F1 close   click outside: world   drag: copy   right click: paste   key -> screen {:.1} ms (worst {:.1})   {:.0} fps{}",
            t.last_ms, t.worst_ms, t.fps, if t.alive { "" } else { "   [shell exited]" }
        )
    } else {
        "[ world ]   T or click here (after Esc): back to terminal   F1 close".to_string()
    };
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
fn auto_type(time: Res<Time>, mut t: ResMut<Terminal>, mut done: Local<bool>, mut clipboard: ResMut<Clipboard>) {
    let at = std::env::var("AT").ok().and_then(|v| v.parse().ok()).unwrap_or(3.5);
    if *done || time.elapsed_secs() < at {
        return;
    }
    *done = true;
    if std::env::var("CLIP_TEST").is_ok() {
        eprintln!("clip test: set {:?}", clipboard.set_text("clip-test"));
        eprintln!("clip test: get {:?}", clipboard.fetch_text().poll_result());
        t.open = true;
        t.focus = true;
        paste(&mut t, &mut clipboard);
    }
    if let Ok(text) = std::env::var("TYPE") {
        t.open = true;
        t.focus = std::env::var("UNFOCUS").is_err();
        t.dirty = true;
        t.link.send(text.replace("\\n", "\r").into_bytes());
        t.sent_at = Some(Instant::now());
    }
}
