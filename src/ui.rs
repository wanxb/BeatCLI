use crate::player;
use crate::playlist::PlaybackMode;
use crate::playlist::PlaylistView;
use crossterm::execute;
use crossterm::style::{Color, Print, ResetColor, SetForegroundColor};
use std::io::{Write, stdout};

#[derive(Clone, Default)]
pub struct UiState {
    pub show_welcome: bool,
    pub flash: Option<(String, FlashLevel)>, // 新增 FlashLevel
    pub now_index: Option<usize>,
    pub now_name: String,
    pub next_name: String,
    pub volume: Option<u8>,
    pub mode: PlaybackMode,

    // 新增歌词相关
    pub lyrics: Option<crate::lyrics::Lyrics>,
    // 当前播放时间（毫秒）
    pub current_ms: u128,
}

#[derive(Clone)]
pub enum FlashLevel {
    Info,
    Ok,
    Error,
}

impl Default for FlashLevel {
    fn default() -> Self {
        FlashLevel::Info
    }
}

impl UiState {
    pub fn set_now_playing(&mut self, idx: usize, name: String, next: String) {
        self.now_index = Some(idx);
        self.now_name = name;
        self.next_name = next;
        self.show_welcome = false;
    }

    pub fn flash_message(&mut self, msg: Option<String>, level: FlashLevel) {
        self.flash = msg.map(|s| (s, level));
    }
}

pub struct Screen;

impl Screen {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self)
    }

    pub fn draw(&mut self, ui: &mut UiState, pl: &PlaylistView) -> std::io::Result<()> {
        let mut out = String::new();

        // 欢迎页
        if ui.show_welcome {
            out.push_str("========================================\n");
            out.push_str("     🎵 BeatCLI — Console Music Player\n");
            out.push_str("========================================\n\n");
            out.push_str("输入 /help 查看命令，/folder <path> 选择音乐目录\n\n");
        }

        // 仅在有歌曲播放时显示状态
        if ui.now_index.is_some() {
            let now = if ui.now_name.is_empty() {
                "(未播放)".to_string()
            } else {
                ui.now_name.clone()
            };
            let next = if ui.next_name.is_empty() {
                "(无)".to_string()
            } else {
                ui.next_name.clone()
            };
            out.push_str("\n=================================================\n");
            out.push_str(&format!("当前播放: {}\n", now));
            out.push_str(&format!("下一首: {}\n\n", next));
            out.push_str(&format!(
                "播放模式: {}    音量: {}%    播放列表: {} 首\n",
                match ui.mode {
                    PlaybackMode::Sequential => "顺序播放",
                    PlaybackMode::RepeatOne => "单曲循环",
                    PlaybackMode::Shuffle => "随机播放",
                },
                ui.volume.unwrap_or(50),
                pl.len
            ));
            out.push_str("=================================================\n");
            // 歌词显示
            if let Some(lyrics) = &ui.lyrics {
                if !lyrics.lines.is_empty() {
                    let current_ms = ui.current_ms; // ✅ 使用 UiState 中的播放进度
                    let current_idx = lyrics.current_line_index(current_ms);
                    let start = current_idx.saturating_sub(3);
                    let end = (current_idx + 4).min(lyrics.lines.len());

                    out.push('\n');
                    for i in start..end {
                        let (_, ref text) = lyrics.lines[i];
                        if i == current_idx {
                            execute!(
                                stdout(),
                                SetForegroundColor(Color::Green),
                                Print(format!("> {}\n", text)),
                                ResetColor
                            )?;
                        } else {
                            out.push_str(&format!("  {}\n", text));
                        }
                    }
                }
            }
        }

        if let Some((msg, level)) = &ui.flash {
            let prefix = match level {
                FlashLevel::Info => ("[Info] ", Color::Blue),
                FlashLevel::Ok => ("[OK] ", Color::Green),
                FlashLevel::Error => ("[Error] ", Color::Red),
            };

            let mut stdout = stdout();
            execute!(
                stdout,
                SetForegroundColor(prefix.1),
                Print(prefix.0),
                ResetColor,
                Print(msg)
            )?;
            ui.flash = None; // 显示后清除
        }
        out.push_str("\n>>: "); // 提示符
        print!("{}", out);
        stdout().flush()
    }
}
