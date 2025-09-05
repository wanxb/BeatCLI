mod command;
mod lyrics;
mod player;
mod playlist;
mod ui;

use crate::command::{Command, parse_command};
use crate::player::Player;
use crate::playlist::{PlaybackMode, Playlist};
use crate::ui::{FlashLevel, Screen, UiState};

use crossbeam_channel::{Receiver, Sender, unbounded};
use parking_lot::Mutex;
use std::{
    io::{self, BufRead, Write},
    sync::Arc,
    thread,
    time::Duration,
};

#[derive(Clone)]
struct SharedState {
    ui: Arc<Mutex<UiState>>,
    playlist: Arc<Mutex<Playlist>>,
}

fn main() -> anyhow::Result<()> {
    let ui_state = Arc::new(Mutex::new(UiState::default()));
    let playlist = Arc::new(Mutex::new(Playlist::default()));
    let shared = SharedState {
        ui: ui_state.clone(),
        playlist: playlist.clone(),
    };

    let (tx, rx): (Sender<Command>, Receiver<Command>) = unbounded();

    // 播放线程
    {
        let s = shared.clone();
        let rx_player = rx.clone();
        thread::spawn(move || {
            let mut player = Player::new().expect("播放器初始化失败！");
            playback_loop(s, rx_player, &mut player);
        });
    }

    // 欢迎信息打印一次
    println!("{}", help_text());

    // 输入线程在 main 线程
    input_loop(shared, tx)?;

    Ok(())
}

// 主播放循环
fn playback_loop(shared: SharedState, rx: Receiver<Command>, player: &mut Player) {
    loop {
        // 处理命令
        loop {
            if let Ok(cmd) = rx.try_recv() {
                if let Command::Quit = cmd {
                    return;
                }
                apply_command(&shared, player, cmd);
            } else {
                break;
            }
        }

        // 播放完成处理
        if player.finished() {
            let mut pl = shared.playlist.lock();
            if let Some(next_idx) = pl.advance_on_finished() {
                let path = pl.items[next_idx].clone();
                player.play_file(&path);

                // 保持音量
                let vol = shared.ui.lock().volume.unwrap_or(50) as f32 / 100.0;
                player.set_volume(vol);

                let mut ui = shared.ui.lock();
                ui.set_now_playing(
                    next_idx,
                    path.file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string(),
                    pl.peek_next_name(),
                );
                ui.current_ms = 0; // 重置播放进度
            }
        } else {
            // 更新当前播放时间
            shared.ui.lock().current_ms = player.get_current_ms();
        }

        thread::sleep(Duration::from_millis(120));
    }
}

// 应用命令
fn apply_command(shared: &SharedState, player: &mut Player, cmd: Command) {
    match cmd {
        Command::Help => flash(shared, help_text(), FlashLevel::Info),

        Command::Quit => {} // 主循环会处理退出

        Command::Folder(path) => {
            let mut pl = shared.playlist.lock();
            match pl.scan_folder(&path) {
                Ok(count) => flash(shared, format!("扫描到 {count} 首歌曲"), FlashLevel::Info),
                Err(e) => flash(shared, format!("扫描失败: {}", e), FlashLevel::Error),
            }
        }

        Command::List => {
            let pl = shared.playlist.lock();
            if pl.items.is_empty() {
                flash(
                    shared,
                    "(空播放列表)\n请先使用 /folder <path> 选择目录".to_string(),
                    FlashLevel::Info,
                );
            } else {
                let mut msg = "播放列表:\n".to_string();
                for (i, path, is_current) in pl.list() {
                    let name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("(未知文件名)");
                    msg.push_str(&format_item(i, name, is_current));
                }
                flash(shared, msg, FlashLevel::Info);
            }
        }

        Command::Search(query) => {
            let pl = shared.playlist.lock();
            if pl.items.is_empty() {
                flash(
                    shared,
                    "播放列表为空，请先使用 /folder 添加歌曲".to_string(),
                    FlashLevel::Error,
                );
                return;
            }

            let results = pl.search(&query);
            if results.is_empty() {
                flash(
                    shared,
                    format!("未找到包含 '{}' 的歌曲", query),
                    FlashLevel::Info,
                );
            } else {
                let mut msg = format!("找到 {} 首包含 '{}' 的歌曲:\n", results.len(), query);
                let current_idx = pl.current;
                for (i, path) in results.iter() {
                    let name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("(未知文件名)");
                    let is_current = Some(*i) == current_idx;
                    msg.push_str(&format_item(*i, name, is_current));
                }
                flash(shared, msg, FlashLevel::Info);
            }
        }

        Command::PlayIndex(mut i) => {
            let pl_len = shared.playlist.lock().items.len();
            if pl_len == 0 {
                flash(
                    shared,
                    "播放列表为空，请先使用 /folder 添加歌曲".to_string(),
                    FlashLevel::Error,
                );
                return;
            }

            // 默认播放第一首
            if i == 0 || i > pl_len {
                i = 0;
            } else {
                i -= 1;
            }

            play_song(shared, player, i);
        }

        Command::Pause => {
            if check_playlist_empty(shared) {
                return;
            }
            if !is_playing(shared) {
                flash(shared, "没有正在播放的歌曲".to_string(), FlashLevel::Error);
                return;
            }
            player.pause();
            flash(shared, "已暂停".to_string(), FlashLevel::Ok);
        }

        Command::Resume => {
            if check_playlist_empty(shared) {
                return;
            }
            if !is_playing(shared) {
                flash(shared, "没有正在播放的歌曲".to_string(), FlashLevel::Error);
                return;
            }
            player.resume();
            flash(shared, "继续播放".to_string(), FlashLevel::Ok);
        }

        Command::Next => {
            if check_playlist_empty(shared) {
                return;
            }
            next_song(shared, player);
        }

        Command::Prev => {
            if check_playlist_empty(shared) {
                return;
            }
            prev_song(shared, player);
        }

        Command::Mode(m) => {
            if check_playlist_empty(shared) {
                return;
            }
            let mut pl = shared.playlist.lock();
            match m {
                PlaybackMode::Sequential => {
                    pl.mode = PlaybackMode::Sequential;
                    flash(shared, "已切换到顺序播放模式".to_string(), FlashLevel::Ok);
                }
                PlaybackMode::RepeatOne => {
                    pl.mode = PlaybackMode::RepeatOne;
                    flash(shared, "已切换到单曲循环模式".to_string(), FlashLevel::Ok);
                }
                PlaybackMode::Shuffle => {
                    pl.mode = PlaybackMode::Shuffle;
                    flash(shared, "已切换到随机播放模式".to_string(), FlashLevel::Ok);
                }
                _ => {
                    flash(shared, "未知播放模式".to_string(), FlashLevel::Error);
                }
            }
            shared.ui.lock().mode = m;
        }

        Command::Volume(v) => {
            if check_playlist_empty(shared) {
                return;
            }
            let vol = (v as f32 / 100.0).clamp(0.0, 1.0);
            player.set_volume(vol);
            shared.ui.lock().volume = Some(v);
            flash(shared, format!("音量设置为: {}%", v), FlashLevel::Ok);
        }

        Command::Unknown(s) => {
            flash(
                shared,
                format!("未知命令: {}\n输入 /help 查看帮助。", s),
                FlashLevel::Error,
            );
        }

        _ => {}
    }

    // 每次命令执行后刷新一次 UI
    refresh_ui(shared);
}

// 判断播放列表是否为空
fn check_playlist_empty(shared: &SharedState) -> bool {
    let pl = shared.playlist.lock();
    if pl.items.is_empty() {
        flash(
            shared,
            "播放列表为空，请先使用 /folder 添加歌曲".to_string(),
            FlashLevel::Error,
        );
        true
    } else {
        false
    }
}

// 判断当前是否有歌曲正在播放
fn is_playing(shared: &SharedState) -> bool {
    shared.playlist.lock().current.is_some()
}

// 播放指定索引歌曲
fn play_song(shared: &SharedState, player: &mut Player, i: usize) {
    let path_opt = shared.playlist.lock().get(i).cloned();
    if let Some(path) = path_opt {
        shared.playlist.lock().current = Some(i);
        player.play_file(&path);

        // 保持音量
        let vol = shared.ui.lock().volume.unwrap_or(50) as f32 / 100.0;
        player.set_volume(vol);

        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let next = shared.playlist.lock().peek_next_name();
        shared.ui.lock().set_now_playing(i, name, next);
    }
}

// 播放下一首
fn next_song(shared: &SharedState, player: &mut Player) {
    let mut pl = shared.playlist.lock();
    if let Some(next_idx) = pl.next_index() {
        let path = pl.get(next_idx).cloned().unwrap();
        pl.current = Some(next_idx);
        player.play_file(&path);

        // 保持音量
        let vol = shared.ui.lock().volume.unwrap_or(50) as f32 / 100.0;
        player.set_volume(vol);

        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let next = pl.peek_next_name();
        shared.ui.lock().set_now_playing(next_idx, name, next);
    } else {
        flash(shared, "已经是最后一首".to_string(), FlashLevel::Info);
    }
}

// 播放上一首
fn prev_song(shared: &SharedState, player: &mut Player) {
    let pl = shared.playlist.lock();
    if let Some(prev_idx) = pl.prev_index() {
        let path = pl.get(prev_idx).cloned().unwrap();
        drop(pl); // 释放锁
        shared.playlist.lock().current = Some(prev_idx);
        player.play_file(&path);

        // 保持音量
        let vol = shared.ui.lock().volume.unwrap_or(50) as f32 / 100.0;
        player.set_volume(vol);

        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let next = shared.playlist.lock().peek_next_name();
        shared.ui.lock().set_now_playing(prev_idx, name, next);
    } else {
        flash(shared, "已经是第一首".to_string(), FlashLevel::Info);
    }
}

// 刷新 UI
fn refresh_ui(shared: &SharedState) {
    let mut ui_lock = shared.ui.lock();
    let pl_view = shared.playlist.lock().clone_view();
    if let Ok(mut screen) = Screen::new() {
        screen.draw(&mut *ui_lock, &pl_view).ok();
    }
}

// 通用 flash 输出
fn flash<T: Into<String>>(shared: &SharedState, msg: T, level: FlashLevel) {
    shared.ui.lock().flash_message(Some(msg.into()), level);
}

// 输入线程
fn input_loop(shared: SharedState, tx: Sender<Command>) -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut stdin_lock = stdin.lock();
    print!(">>: ");
    loop {
        std::io::stdout().flush().ok();

        let mut line = String::new();
        let n = stdin_lock.read_line(&mut line)?;
        if n == 0 {
            break;
        } // EOF

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // 用户回车后换行，保证 flash 输出在新行
        println!();

        let cmd = parse_command(line);
        let _ = tx.send(cmd.clone());
        shared.ui.lock().show_welcome = false;

        if matches!(cmd, Command::Quit) {
            break;
        }
    }

    Ok(())
}

// 帮助文本
fn help_text() -> String {
    let mut s = String::new();
    s.push_str("========================================\n");
    s.push_str("     🎵 BeatCLI — Console Music Player\n");
    s.push_str("========================================\n\n");
    s.push_str("常用命令:\n");
    s.push_str("/help                显示帮助\n");
    s.push_str("/folder <path>       选择音乐文件夹\n");
    s.push_str("/list                列出播放列表\n");
    s.push_str("/play <N>            播放第 N 首(从1开始)，默认播放第一首\n");
    s.push_str("/pause               暂停\n");
    s.push_str("/resume              继续\n");
    s.push_str("/next                下一首\n");
    s.push_str("/prev                上一首\n");
    s.push_str("/mode <Sequential|RepeatOne|Shuffle> 切换播放模式\n");
    s.push_str("/volume <0..100>     设置音量\n");
    s.push_str("/quit                退出\n\n");
    s
}

// 格式化播放列表条目
fn format_item(idx: usize, name: &str, is_current: bool) -> String {
    let marker = if is_current { ">" } else { " " };
    format!("  {}. {}{}\n", idx + 1, marker, name)
}
