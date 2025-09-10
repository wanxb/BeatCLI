mod command;
mod lyrics;
mod player;
mod playlist;
mod ui;

use crate::command::{Command, parse_command};
use crate::lyrics::Lyrics;
use crate::player::Player;
use crate::playlist::{PlaybackMode, Playlist};
use crate::ui::{FlashLevel, Screen, UiState, show_goodbye_message};

use crossbeam_channel::{Receiver, Sender, select, unbounded};
use parking_lot::Mutex;
use std::{
    io::{self, BufRead, Write},
    sync::Arc,
    thread,
    time::Duration,
};

// 应用状态
#[derive(Clone)]
struct AppState {
    ui: Arc<Mutex<UiState>>,
    playlist: Arc<Mutex<Playlist>>,
}

// 应用事件
#[derive(Debug, Clone)]
enum AppEvent {
    // UI事件
    ShowMessage(String, FlashLevel),
    UpdatePlayingState(usize, String, String), // index, current, next
    UpdateLyrics(Option<Lyrics>),
    UpdateProgress(u128),
    RefreshUI,

    // 播放事件
    PlayFile(usize),
    PlayFinished,

    // 系统事件
    Shutdown,
}

fn main() -> anyhow::Result<()> {
    let ui_state = Arc::new(Mutex::new(UiState::default()));
    let playlist = Arc::new(Mutex::new(Playlist::default()));
    let app_state = AppState {
        ui: ui_state.clone(),
        playlist: playlist.clone(),
    };

    let (cmd_tx, cmd_rx): (Sender<Command>, Receiver<Command>) = unbounded();
    let (event_tx, event_rx): (Sender<AppEvent>, Receiver<AppEvent>) = unbounded();

    // 启动播放线程
    {
        let state = app_state.clone();
        let cmd_rx = cmd_rx.clone();
        let event_tx = event_tx.clone();
        thread::spawn(move || {
            let mut player = match Player::new() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("错误: 播放器初始化失败: {}", e);
                    return;
                }
            };
            audio_thread(state, cmd_rx, event_tx, &mut player);
        });
    }

    // 启动UI刷新线程
    {
        let state = app_state.clone();
        let event_rx = event_rx.clone();
        thread::spawn(move || {
            ui_thread(state, event_rx);
        });
    }

    // 显示初始欢迎信息
    println!("{}", help_text());

    // 主线程处理用户输入
    input_thread(app_state, cmd_tx, event_tx)?;

    Ok(())
}

// 音频播放线程
fn audio_thread(
    state: AppState,
    cmd_rx: Receiver<Command>,
    event_tx: Sender<AppEvent>,
    player: &mut Player,
) {
    loop {
        select! {
            recv(cmd_rx) -> cmd => {
                match cmd {
                    Ok(Command::Quit) => {
                        let _ = event_tx.send(AppEvent::Shutdown);
                        break;
                    }
                    Ok(command) => {
                        handle_command(&state, player, command, &event_tx);
                    }
                    Err(_) => break, // Channel closed
                }
            }
            default(Duration::from_millis(200)) => {
                // 检查播放状态
                if player.finished() {
                    let mut pl = state.playlist.lock();
                    if let Some(next_idx) = pl.advance_on_finished() {
                        let path = pl.items[next_idx].clone();
                        drop(pl);

                        player.play_file(&path);
                        let vol = state.ui.lock().volume.unwrap_or(50) as f32 / 100.0;
                        player.set_volume(vol);

                        let name = path.file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_string();
                        let next_name = state.playlist.lock().peek_next_name();
                        let lyrics = Lyrics::load_from_path(&path);

                        // 发送UI更新事件
                        let _ = event_tx.send(AppEvent::UpdatePlayingState(next_idx, name, next_name));
                        let _ = event_tx.send(AppEvent::UpdateLyrics(lyrics));
                        let _ = event_tx.send(AppEvent::RefreshUI);
                    }
                } else {
                    // 更新播放进度
                    let current_ms = player.get_current_ms();
                    let _ = event_tx.send(AppEvent::UpdateProgress(current_ms));

                    // 检查歌词是否需要更新定位（只在歌词行切换时才刷新UI）
                    let ui = state.ui.lock();
                    if ui.show_lyrics && ui.lyrics.is_some() && ui.now_index.is_some() {
                        if let Some(lyrics) = &ui.lyrics {
                            let new_line_idx = lyrics.current_line_index(current_ms);
                            let old_line_idx = ui.current_lyric_line.unwrap_or(usize::MAX);

                            // 只有当歌词行发生变化时才刷新UI
                            if new_line_idx != old_line_idx {
                                drop(ui);
                                // 更新当前歌词行索引
                                state.ui.lock().current_lyric_line = Some(new_line_idx);
                                let _ = event_tx.send(AppEvent::RefreshUI);
                            }
                        }
                    }
                }
            }
        }
    }
}

// UI线程
fn ui_thread(state: AppState, event_rx: Receiver<AppEvent>) {
    loop {
        match event_rx.recv() {
            Ok(AppEvent::ShowMessage(msg, level)) => {
                state.ui.lock().flash_message(Some(msg), level);
                refresh_ui_now(&state);
            }
            Ok(AppEvent::UpdatePlayingState(idx, current, next)) => {
                let mut ui = state.ui.lock();
                ui.set_now_playing(idx, current, next);
                ui.show_welcome = false;
                // 不在这里刷新UI，等待ShowMessage事件一起刷新
            }
            Ok(AppEvent::UpdateLyrics(lyrics)) => {
                state.ui.lock().lyrics = lyrics;
            }
            Ok(AppEvent::UpdateProgress(ms)) => {
                state.ui.lock().current_ms = ms;
                // 不自动刷新UI，只有在歌词行变化时才刷新
            }
            Ok(AppEvent::RefreshUI) => {
                // 对于 RefreshUI 事件，强制刷新播放界面
                let mut ui = state.ui.lock();
                if ui.playing_ui_active {
                    let pl_view = state.playlist.lock().clone_view();
                    if let Ok(mut screen) = Screen::new() {
                        let _ = screen.force_refresh_playing_interface(&mut *ui, &pl_view);
                    }
                } else {
                    drop(ui);
                    refresh_ui_now(&state);
                }
            }
            Ok(AppEvent::Shutdown) => {
                show_goodbye_message();
                break;
            }
            _ => break,
        }
    }
}

// 输入线程
fn input_thread(
    state: AppState,
    cmd_tx: Sender<Command>,
    event_tx: Sender<AppEvent>,
) -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut stdin_lock = stdin.lock();

    loop {
        // 只有在欢迎页或非播放模式下才显示输入提示符
        let ui = state.ui.lock();
        let should_show_prompt = ui.show_welcome || !ui.playing_ui_active;
        drop(ui);

        if should_show_prompt {
            print!(">>: ");
            std::io::stdout().flush().ok();
        }

        let mut line = String::new();
        let n = stdin_lock.read_line(&mut line)?;
        if n == 0 {
            break;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let command = parse_command(line);

        if matches!(command, Command::Quit) {
            let _ = cmd_tx.send(command);
            break;
        }

        let _ = cmd_tx.send(command);

        // 给命令处理一些时间
        thread::sleep(Duration::from_millis(100));
    }

    Ok(())
}

// 处理命令
fn handle_command(
    state: &AppState,
    player: &mut Player,
    cmd: Command,
    event_tx: &Sender<AppEvent>,
) {
    match cmd {
        Command::Help => {
            let _ = event_tx.send(AppEvent::ShowMessage(help_text(), FlashLevel::Info));
        }

        Command::Folder(path) => {
            // 验证路径
            if path.trim().is_empty() {
                let _ = event_tx.send(AppEvent::ShowMessage(
                    "路径不能为空，请指定有效的文件夹路径".to_string(),
                    FlashLevel::Error,
                ));
                return;
            }

            let folder_path = std::path::Path::new(&path);
            if !folder_path.exists() {
                let _ = event_tx.send(AppEvent::ShowMessage(
                    format!("路径不存在: {}", path),
                    FlashLevel::Error,
                ));
                return;
            }

            if !folder_path.is_dir() {
                let _ = event_tx.send(AppEvent::ShowMessage(
                    format!("路径不是一个文件夹: {}", path),
                    FlashLevel::Error,
                ));
                return;
            }

            let mut pl = state.playlist.lock();
            match pl.scan_folder(&path) {
                Ok(count) => {
                    if count == 0 {
                        let _ = event_tx.send(AppEvent::ShowMessage(
                            format!("文件夹 '{}' 中没有找到支持的音频文件", path),
                            FlashLevel::Info,
                        ));
                    } else {
                        let _ = event_tx.send(AppEvent::ShowMessage(
                            format!("扫描到 {} 首歌曲", count),
                            FlashLevel::Ok,
                        ));
                    }
                }
                Err(e) => {
                    let _ = event_tx.send(AppEvent::ShowMessage(
                        format!("扫描失败: {}", e),
                        FlashLevel::Error,
                    ));
                }
            }
        }

        Command::List => {
            let pl = state.playlist.lock();
            if pl.items.is_empty() {
                let _ = event_tx.send(AppEvent::ShowMessage(
                    "(空播放列表)\n请先使用 /folder <path> 选择目录".to_string(),
                    FlashLevel::Info,
                ));
            } else {
                let mut msg = "播放列表:\n".to_string();
                for (i, path, is_current) in pl.list() {
                    let name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("(未知文件名)");
                    msg.push_str(&format_item(i, name, is_current));
                }
                let _ = event_tx.send(AppEvent::ShowMessage(msg, FlashLevel::Info));
            }
        }

        Command::PlayIndex(mut i) => {
            let pl_len = state.playlist.lock().items.len();
            if pl_len == 0 {
                let _ = event_tx.send(AppEvent::ShowMessage(
                    "播放列表为空，请先使用 /folder 添加歌曲".to_string(),
                    FlashLevel::Error,
                ));
                return;
            }

            if i > pl_len {
                let _ = event_tx.send(AppEvent::ShowMessage(
                    format!(
                        "歌曲序号超出范围，当前播放列表有 {} 首歌曲，请输入 1-{} 之间的数字",
                        pl_len, pl_len
                    ),
                    FlashLevel::Error,
                ));
                return;
            }

            if i > 0 && i <= pl_len {
                i = i - 1; // 转换为0基索引
            } else {
                i = 0;
            }

            play_song(state, player, i, event_tx);
        }

        Command::Next => {
            if check_playlist_empty(state, event_tx) {
                return;
            }
            next_song(state, player, event_tx);
        }

        Command::Prev => {
            if check_playlist_empty(state, event_tx) {
                return;
            }
            prev_song(state, player, event_tx);
        }

        Command::Pause => {
            if check_playlist_empty(state, event_tx) {
                return;
            }
            if !is_playing(state) {
                let _ = event_tx.send(AppEvent::ShowMessage(
                    "没有正在播放的歌曲".to_string(),
                    FlashLevel::Error,
                ));
                return;
            }
            player.pause();
            let _ = event_tx.send(AppEvent::ShowMessage("已暂停".to_string(), FlashLevel::Ok));
        }

        Command::Resume => {
            if check_playlist_empty(state, event_tx) {
                return;
            }
            if !is_playing(state) {
                let _ = event_tx.send(AppEvent::ShowMessage(
                    "没有正在播放的歌曲".to_string(),
                    FlashLevel::Error,
                ));
                return;
            }
            player.resume();
            let _ = event_tx.send(AppEvent::ShowMessage(
                "继续播放".to_string(),
                FlashLevel::Ok,
            ));
        }

        Command::Volume(v) => {
            if check_playlist_empty(state, event_tx) {
                return;
            }
            if !is_playing(state) {
                let _ = event_tx.send(AppEvent::ShowMessage(
                    "当前没有播放歌曲，无法调节音量".to_string(),
                    FlashLevel::Error,
                ));
                return;
            }
            let vol = (v as f32 / 100.0).clamp(0.0, 1.0);
            player.set_volume(vol);
            state.ui.lock().volume = Some(v);
            let _ = event_tx.send(AppEvent::ShowMessage(
                format!("音量设置为: {}%", v),
                FlashLevel::Ok,
            ));
        }

        Command::Lyrics => {
            if !is_playing(state) {
                let _ = event_tx.send(AppEvent::ShowMessage(
                    "当前没有播放歌曲，无法操作歌词显示".to_string(),
                    FlashLevel::Error,
                ));
                return;
            }

            let mut ui = state.ui.lock();
            ui.toggle_lyrics();
            let status = if ui.show_lyrics {
                "已显示"
            } else {
                "已隐藏"
            };

            if ui.show_lyrics {
                if let Some(lyrics) = &ui.lyrics {
                    if lyrics.lines.is_empty() {
                        let _ = event_tx.send(AppEvent::ShowMessage(
                            format!("歌词{}，但歌词文件为空", status),
                            FlashLevel::Info,
                        ));
                    } else {
                        let _ = event_tx.send(AppEvent::ShowMessage(
                            format!("歌词{}，已加载 {} 行歌词", status, lyrics.lines.len()),
                            FlashLevel::Ok,
                        ));
                    }
                } else {
                    let _ = event_tx.send(AppEvent::ShowMessage(
                        format!("歌词{}，但未找到歌词文件", status),
                        FlashLevel::Info,
                    ));
                }
            } else {
                let _ = event_tx.send(AppEvent::ShowMessage(
                    format!("歌词{}", status),
                    FlashLevel::Ok,
                ));
            }
            let _ = event_tx.send(AppEvent::RefreshUI);
        }

        Command::LyricsMode => {
            if !is_playing(state) {
                let _ = event_tx.send(AppEvent::ShowMessage(
                    "当前没有播放歌曲，无法切换歌词显示模式".to_string(),
                    FlashLevel::Error,
                ));
                return;
            }

            let mut ui = state.ui.lock();
            ui.toggle_lyrics_mode();
            let mode_name = if ui.lyrics_stream_mode {
                "流式输出"
            } else {
                "清屏刷新"
            };
            
            let _ = event_tx.send(AppEvent::ShowMessage(
                format!("歌词显示模式已切换为: {}", mode_name),
                FlashLevel::Ok,
            ));
            let _ = event_tx.send(AppEvent::RefreshUI);
        }

        Command::Now => {
            if check_playlist_empty(state, event_tx) {
                return;
            }
            show_now_playing(state, event_tx);
        }

        Command::Search(query) => {
            if check_playlist_empty(state, event_tx) {
                return;
            }

            let pl = state.playlist.lock();
            let results = pl.search(&query);
            drop(pl);

            if results.is_empty() {
                let _ = event_tx.send(AppEvent::ShowMessage(
                    format!("没有找到包含 '{}' 的歌曲", query),
                    FlashLevel::Info,
                ));
            } else {
                let mut msg = format!("搜索 '{}' 的结果：\n", query);
                for (idx, path) in results {
                    let name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("未知文件名");
                    msg.push_str(&format!("  {}. {}\n", idx + 1, name));
                }
                msg.push_str("\n使用 /play <N> 播放指定歌曲");
                let _ = event_tx.send(AppEvent::ShowMessage(msg, FlashLevel::Info));
            }
        }

        Command::Mode(mode) => {
            if check_playlist_empty(state, event_tx) {
                return;
            }

            let mut pl = state.playlist.lock();
            let mode_name = match mode {
                PlaybackMode::Sequential => "顺序播放模式",
                PlaybackMode::RepeatOne => "单曲循环模式",
                PlaybackMode::Shuffle => "随机播放模式",
            };

            // 检查是否已经是该模式
            if pl.mode == mode {
                let _ = event_tx.send(AppEvent::ShowMessage(
                    format!("已经是{}", mode_name),
                    FlashLevel::Info,
                ));
                return;
            }

            pl.mode = mode;
            state.ui.lock().mode = mode;
            drop(pl);
            
            let _ = event_tx.send(AppEvent::ShowMessage(
                format!("已切换到{}", mode_name),
                FlashLevel::Ok,
            ));
        }

        Command::Quit => {
            // Quit 已在 audio_thread 中处理
        }

        Command::Unknown(s) => {
            let _ = event_tx.send(AppEvent::ShowMessage(
                format!("未知命令: {}\n输入 /help 查看帮助。", s),
                FlashLevel::Error,
            ));
        }
    }
}

// 辅助函数
fn check_playlist_empty(state: &AppState, event_tx: &Sender<AppEvent>) -> bool {
    let pl = state.playlist.lock();
    if pl.items.is_empty() {
        let _ = event_tx.send(AppEvent::ShowMessage(
            "播放列表为空，请先使用 /folder 添加歌曲".to_string(),
            FlashLevel::Error,
        ));
        true
    } else {
        false
    }
}

fn is_playing(state: &AppState) -> bool {
    state.playlist.lock().current.is_some()
}

fn play_song(state: &AppState, player: &mut Player, i: usize, event_tx: &Sender<AppEvent>) {
    let path_opt = state.playlist.lock().get(i).cloned();
    if let Some(path) = path_opt {
        if !path.exists() {
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("未知文件");
            let _ = event_tx.send(AppEvent::ShowMessage(
                format!("歌曲文件不存在: {}", name),
                FlashLevel::Error,
            ));
            return;
        }

        state.playlist.lock().current = Some(i);
        player.play_file(&path);

        let vol = state.ui.lock().volume.unwrap_or(50) as f32 / 100.0;
        player.set_volume(vol);

        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let next = state.playlist.lock().peek_next_name();
        let lyrics = Lyrics::load_from_path(&path);

        // 发送更新事件
        let _ = event_tx.send(AppEvent::UpdatePlayingState(i, name.clone(), next));
        let _ = event_tx.send(AppEvent::UpdateLyrics(lyrics.clone()));

        let mut flash_msg = format!("开始播放: {}", name);
        if lyrics.is_some() {
            flash_msg.push_str(" | 已加载歌词");
        }
        let _ = event_tx.send(AppEvent::ShowMessage(flash_msg, FlashLevel::Ok));
    }
}

fn next_song(state: &AppState, player: &mut Player, event_tx: &Sender<AppEvent>) {
    let mut pl = state.playlist.lock();

    if pl.items.len() == 1 {
        let _ = event_tx.send(AppEvent::ShowMessage(
            "只有一首歌曲，无法切换到下一首".to_string(),
            FlashLevel::Info,
        ));
        return;
    }

    if let Some(next_idx) = pl.next_index() {
        let path = pl.get(next_idx).cloned().unwrap();
        pl.current = Some(next_idx);
        drop(pl);

        player.play_file(&path);
        let vol = state.ui.lock().volume.unwrap_or(50) as f32 / 100.0;
        player.set_volume(vol);

        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let next = state.playlist.lock().peek_next_name();
        let lyrics = Lyrics::load_from_path(&path);

        let _ = event_tx.send(AppEvent::UpdatePlayingState(next_idx, name.clone(), next));
        let _ = event_tx.send(AppEvent::UpdateLyrics(lyrics));
        let _ = event_tx.send(AppEvent::ShowMessage(
            format!("已切换到下一首: {}", name),
            FlashLevel::Ok,
        ));
    } else {
        let mode = state.playlist.lock().mode;
        match mode {
            PlaybackMode::Sequential => {
                let _ = event_tx.send(AppEvent::ShowMessage(
                    "已经是最后一首，顺序播放模式下不循环".to_string(),
                    FlashLevel::Info,
                ));
            }
            _ => {
                let _ = event_tx.send(AppEvent::ShowMessage(
                    "无法获取下一首歌曲".to_string(),
                    FlashLevel::Error,
                ));
            }
        }
    }
}

fn prev_song(state: &AppState, player: &mut Player, event_tx: &Sender<AppEvent>) {
    let pl = state.playlist.lock();

    if pl.items.len() == 1 {
        let _ = event_tx.send(AppEvent::ShowMessage(
            "只有一首歌曲，无法切换到上一首".to_string(),
            FlashLevel::Info,
        ));
        return;
    }

    if let Some(prev_idx) = pl.prev_index() {
        let path = pl.get(prev_idx).cloned().unwrap();
        drop(pl);
        state.playlist.lock().current = Some(prev_idx);
        player.play_file(&path);

        let vol = state.ui.lock().volume.unwrap_or(50) as f32 / 100.0;
        player.set_volume(vol);

        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let next = state.playlist.lock().peek_next_name();
        let lyrics = Lyrics::load_from_path(&path);

        let _ = event_tx.send(AppEvent::UpdatePlayingState(prev_idx, name.clone(), next));
        let _ = event_tx.send(AppEvent::UpdateLyrics(lyrics));
        let _ = event_tx.send(AppEvent::ShowMessage(
            format!("已切换到上一首: {}", name),
            FlashLevel::Ok,
        ));
    } else {
        let mode = state.playlist.lock().mode;
        match mode {
            PlaybackMode::Sequential => {
                let _ = event_tx.send(AppEvent::ShowMessage(
                    "已经是第一首，顺序播放模式下不循环".to_string(),
                    FlashLevel::Info,
                ));
            }
            _ => {
                let _ = event_tx.send(AppEvent::ShowMessage(
                    "无法获取上一首歌曲".to_string(),
                    FlashLevel::Error,
                ));
            }
        }
    }
}

fn show_now_playing(state: &AppState, event_tx: &Sender<AppEvent>) {
    let ui = state.ui.lock();
    let pl = state.playlist.lock();

    if let Some(current_idx) = pl.current {
        let mut info = String::new();

        info.push_str(&"═".repeat(60));
        info.push_str("\n");
        info.push_str(&format!("{:^60}\n", "🎵 当前播放信息"));
        info.push_str(&"═".repeat(60));
        info.push_str("\n\n");

        info.push_str(&"─".repeat(20));
        info.push_str(" 基本信息 ");
        info.push_str(&"─".repeat(19));
        info.push_str("\n");

        info.push_str(&format!("  歌曲: {}\n", ui.now_name));
        info.push_str(&format!(
            "  序号: {} / {}\n",
            current_idx + 1,
            pl.items.len()
        ));
        info.push_str(&format!(
            "  模式: {}\n",
            match ui.mode {
                PlaybackMode::Sequential => "顺序播放",
                PlaybackMode::RepeatOne => "单曲循环",
                PlaybackMode::Shuffle => "随机播放",
            }
        ));
        info.push_str(&format!("  音量: {}%\n", ui.volume.unwrap_or(50)));

        let current_ms = ui.current_ms;
        let minutes = current_ms / 60_000;
        let seconds = (current_ms % 60_000) / 1000;
        info.push_str(&format!("  播放时间: {:02}:{:02}\n\n", minutes, seconds));

        info.push_str(&"─".repeat(20));
        info.push_str(" 歌词信息 ");
        info.push_str(&"─".repeat(19));
        info.push_str("\n");

        if ui.show_lyrics {
            if let Some(lyrics) = &ui.lyrics {
                if !lyrics.lines.is_empty() {
                    info.push_str(&format!("  歌词: 已加载 ({} 行)\n\n", lyrics.lines.len()));

                    info.push_str(&"─".repeat(20));
                    info.push_str(" 当前歌词 ");
                    info.push_str(&"─".repeat(19));
                    info.push_str("\n");

                    let current_idx = lyrics.current_line_index(current_ms);
                    let start = current_idx.saturating_sub(2);
                    let end = (current_idx + 3).min(lyrics.lines.len());

                    for i in start..end {
                        let (_, ref text) = lyrics.lines[i];
                        if i == current_idx {
                            info.push_str(&format!("  ▶ {}\n", text));
                        } else {
                            info.push_str(&format!("    {}\n", text));
                        }
                    }
                } else {
                    info.push_str("  歌词: 文件为空\n");
                }
            } else {
                info.push_str("  歌词: 未找到歌词文件\n");
            }
        } else {
            info.push_str("  歌词: 已关闭\n");
        }

        info.push_str("\n");
        info.push_str(&"═".repeat(60));
        info.push_str("\n");

        drop(ui);
        drop(pl);
        let _ = event_tx.send(AppEvent::ShowMessage(info, FlashLevel::Info));
    } else {
        // 简单提示，不显示复杂框架
        let _ = event_tx.send(AppEvent::ShowMessage(
            "当前没有播放歌曲，使用 /play 开始播放".to_string(),
            FlashLevel::Info,
        ));
    }
}

fn refresh_ui_now(state: &AppState) {
    let mut ui_lock = state.ui.lock();
    let pl_view = state.playlist.lock().clone_view();
    if let Ok(mut screen) = Screen::new() {
        let _ = screen.draw(&mut *ui_lock, &pl_view);
    }
}

fn help_text() -> String {
    let mut s = String::new();
    s.push_str(&"═".repeat(60));
    s.push_str("\n");
    s.push_str(&format!("{:^60}\n", "🎵 BeatCLI — Console Music Player"));
    s.push_str(&"═".repeat(60));
    s.push_str("\n\n");

    s.push_str(&"─".repeat(20));
    s.push_str(" 常用命令 ");
    s.push_str(&"─".repeat(20));
    s.push_str("\n");

    s.push_str("/help                显示帮助\n");
    s.push_str("/folder <path>       选择音乐文件夹\n");
    s.push_str("/list                列出播放列表\n");
    s.push_str("/search <keyword>    搜索歌曲\n");
    s.push_str("/play <N>            播放第 N 首(从1开始)，默认播放第一首\n");
    s.push_str("/pause               暂停\n");
    s.push_str("/resume              继续\n");
    s.push_str("/next                下一首\n");
    s.push_str("/prev                上一首\n");
    s.push_str("/mode <Sequential|RepeatOne|Shuffle> 切换播放模式\n");
    s.push_str("/volume <0..100>     设置音量\n");
    s.push_str("/lyrics              切换歌词显示\n");
    s.push_str("/lmode               切换歌词显示模式(流式/清屏)\n");
    s.push_str("/now                 显示当前播放信息\n");
    s.push_str("/quit                退出\n");

    s.push_str(&"═".repeat(60));
    s.push_str("\n\n");
    s
}

fn format_item(idx: usize, name: &str, is_current: bool) -> String {
    let marker = if is_current { ">" } else { " " };
    format!("  {}. {}{}\n", idx + 1, marker, name)
}
