use crate::playlist::{PlaybackMode, PlaylistView};
use crossterm::cursor::MoveTo;
use crossterm::cursor::{RestorePosition, SavePosition};
use crossterm::execute;
use crossterm::style::{Color, Print, ResetColor, SetForegroundColor};
use crossterm::terminal::{Clear, ClearType};
use std::io::{Write, stdout};
use unicode_width::UnicodeWidthStr;

// 统一UI样式常量
const UI_WIDTH: usize = 60;
const UI_BORDER_CHAR: &str = "═";
const UI_CORNER_CHAR: &str = "█";
const UI_TITLE_COLOR: Color = Color::Cyan;
const UI_ACCENT_COLOR: Color = Color::Yellow;
const UI_SUCCESS_COLOR: Color = Color::Green;
const UI_ERROR_COLOR: Color = Color::Red;
const UI_INFO_COLOR: Color = Color::Blue;

#[derive(Clone, Default)]
pub struct UiState {
    pub show_welcome: bool,
    pub flash: Option<(String, FlashLevel)>,
    pub now_index: Option<usize>,
    pub now_name: String,
    pub next_name: String,
    pub volume: Option<u8>,
    pub mode: PlaybackMode,

    // 歌词相关
    pub lyrics: Option<crate::lyrics::Lyrics>,
    pub current_ms: u128,                  // 当前播放时间（毫秒）
    pub show_lyrics: bool,                 // 是否显示歌词
    pub current_lyric_line: Option<usize>, // 当前歌词行索引，用于检测歌词变化

    // 简化的UI状态管理
    pub playing_ui_active: bool, // 是否处于播放界面模式

    // 流式歌词输出状态
    pub lyrics_stream_mode: bool,     // 是否启用流式歌词输出
    pub lyrics_base_row: Option<u16>, // 歌词区域起始行位置
    pub status_base_row: Option<u16>, // 播放状态区域起始行位置
    pub last_lyrics_range: Option<(usize, usize)>, // 上次显示的歌词范围，用于减少不必要的更新
}

#[derive(Clone, Debug)]
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
        self.show_lyrics = true; // 默认显示歌词
        self.current_lyric_line = None; // 重置歌词行索引
        self.playing_ui_active = true; // 激活播放界面模式

        // 初始化流式输出状态
        self.lyrics_stream_mode = true; // 默认启用流式歌词
        self.lyrics_base_row = None;
        self.status_base_row = None;
        self.last_lyrics_range = None;
    }

    pub fn flash_message(&mut self, msg: Option<String>, level: FlashLevel) {
        self.flash = msg.map(|s| (s, level));
    }

    pub fn toggle_lyrics(&mut self) {
        self.show_lyrics = !self.show_lyrics;
    }

    pub fn clear_flash(&mut self) {
        self.flash = None;
    }

    // 切换歌词显示模式（流式 vs 清屏）
    pub fn toggle_lyrics_mode(&mut self) {
        self.lyrics_stream_mode = !self.lyrics_stream_mode;
        // 切换模式时重置位置信息
        self.lyrics_base_row = None;
        self.status_base_row = None;
        self.last_lyrics_range = None;
    }
}

// 统一UI样式函数
fn create_title_bar(title: &str) -> String {
    let title_width = title.width(); // 使用 unicode-width 计算实际显示宽度
    let total_padding = UI_WIDTH - title_width - 2; // 减去两边的边框字符
    let left_padding = total_padding / 2;
    let right_padding = total_padding - left_padding; // 确保总长度正确

    let mut result = String::new();
    result.push_str(&UI_CORNER_CHAR.repeat(UI_WIDTH));
    result.push('\n');
    result.push_str(&format!(
        "{}{}{}{}",
        UI_CORNER_CHAR,
        " ".repeat(left_padding),
        title,
        " ".repeat(right_padding)
    ));
    result.push_str(UI_CORNER_CHAR);
    result.push('\n');
    result.push_str(&UI_CORNER_CHAR.repeat(UI_WIDTH));
    result.push('\n');
    result
}

fn create_section_header(title: &str) -> String {
    let title_width = title.width(); // 使用 unicode-width 计算实际显示宽度
    let total_border_len = UI_WIDTH - title_width - 2; // 减去两边的空格
    let left_border_len = total_border_len / 2;
    let right_border_len = total_border_len - left_border_len; // 确保总长度正确

    format!(
        "{} {} {}\n",
        UI_BORDER_CHAR.repeat(left_border_len),
        title,
        UI_BORDER_CHAR.repeat(right_border_len)
    )
}

fn create_footer() -> String {
    UI_BORDER_CHAR.repeat(UI_WIDTH) + "\n"
}

fn create_goodbye_message() -> String {
    let mut msg = String::new();
    msg.push_str(&create_title_bar("🎵 感谢使用 BeatCLI"));
    msg.push_str("\n                    再见，下次再见！\n");
    msg.push_str("              希望音乐带给您美好的时光 🎶\n\n");
    msg.push_str(&create_footer());
    msg
}

// 公开的goodbye消息函数
pub fn show_goodbye_message() {
    let mut stdout = stdout();
    execute!(
        stdout,
        SetForegroundColor(UI_TITLE_COLOR),
        Print(create_goodbye_message()),
        ResetColor
    )
    .ok();
}

pub struct Screen;

impl Screen {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self)
    }

    pub fn draw(&mut self, ui: &mut UiState, pl: &PlaylistView) -> std::io::Result<()> {
        let mut stdout = stdout();

        // 欢迎页显示（正常输出）
        if ui.show_welcome {
            let welcome_content = create_title_bar("🎵 BeatCLI — Console Music Player");
            execute!(
                stdout,
                SetForegroundColor(UI_TITLE_COLOR),
                Print(welcome_content),
                ResetColor,
                Print("\n      输入 /help 查看命令，/folder <path> 选择音乐目录\n\n>>： ")
            )?;
            std::io::stdout().flush()?;
            return Ok(());
        }

        // 进入播放模式时清屏并显示播放界面
        if ui.now_index.is_some() && !ui.playing_ui_active {
            execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
            ui.playing_ui_active = true;

            // 显示播放界面，不显示输入提示符
            self.show_playing_interface(ui, pl)?;
            std::io::stdout().flush()?;
            return Ok(());
        }

        // 在播放模式下，检查歌词是否变化
        if ui.playing_ui_active && ui.show_lyrics {
            // 检查歌词是否变化
            if let Some(lyrics) = &ui.lyrics {
                if !lyrics.lines.is_empty() {
                    let current_idx = lyrics.current_line_index(ui.current_ms);
                    let old_idx = ui.current_lyric_line.unwrap_or(usize::MAX);

                    if current_idx != old_idx {
                        ui.current_lyric_line = Some(current_idx);

                        // 根据模式选择不同的刷新方式
                        if ui.lyrics_stream_mode {
                            // 流式输出模式：只更新歌词区域
                            self.stream_update_lyrics(ui, current_idx)?;
                        } else {
                            // 清屏模式：重新显示整个界面
                            execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
                            self.show_playing_interface(ui, pl)?;
                        }

                        std::io::stdout().flush()?;
                        return Ok(());
                    }
                }
            }
        }

        // 显示Flash消息（正常输出）
        if let Some((msg, level)) = &ui.flash {
            let (prefix, color) = match level {
                FlashLevel::Info => ("ℹ ", UI_INFO_COLOR),
                FlashLevel::Ok => ("✓ ", UI_SUCCESS_COLOR),
                FlashLevel::Error => ("✗ ", UI_ERROR_COLOR),
            };

            execute!(
                stdout,
                SetForegroundColor(color),
                Print(prefix),
                ResetColor,
                Print(msg),
                Print("\n")
            )?;

            // 在播放模式下显示输入提示符
            if ui.playing_ui_active {
                print!(">>： ");
            }

            ui.flash = None;
        }

        std::io::stdout().flush()
    }

    // 显示完整的播放界面
    fn show_playing_interface(&self, ui: &UiState, pl: &PlaylistView) -> std::io::Result<()> {
        let mut stdout = stdout();

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

        // 播放状态区域
        let status_content = format!(
            "{}\n  当前播放: {}\n  下一首:   {}\n\n  播放模式: {}    音量: {}%    播放列表: {} 首\n{}",
            create_section_header("🎵 播放状态"),
            now,
            next,
            match ui.mode {
                PlaybackMode::Sequential => "顺序播放",
                PlaybackMode::RepeatOne => "单曲循环",
                PlaybackMode::Shuffle => "随机播放",
            },
            ui.volume.unwrap_or(50),
            pl.len,
            create_footer()
        );

        execute!(
            stdout,
            SetForegroundColor(UI_TITLE_COLOR),
            Print(status_content),
            ResetColor
        )?;

        // 歌词区域
        if ui.show_lyrics {
            if let Some(lyrics) = &ui.lyrics {
                if !lyrics.lines.is_empty() {
                    let current_ms = ui.current_ms;
                    let current_idx = lyrics.current_line_index(current_ms);
                    let start = current_idx.saturating_sub(3);
                    let end = (current_idx + 4).min(lyrics.lines.len());

                    let mut lyrics_content = String::new();
                    lyrics_content.push_str(&create_section_header("🎶 歌词"));

                    for i in start..end {
                        let (_, ref text) = lyrics.lines[i];
                        if i == current_idx {
                            lyrics_content.push_str(&format!("  \x1b[32m▶ {}\x1b[0m\n", text)); // 绿色高亮
                        } else {
                            lyrics_content.push_str(&format!("    {}\n", text));
                        }
                    }

                    lyrics_content.push_str(&create_footer());

                    execute!(
                        stdout,
                        SetForegroundColor(UI_INFO_COLOR),
                        Print(lyrics_content),
                        ResetColor
                    )?;
                }
            }
        }

        Ok(())
    }

    // 流式更新歌词（高度优化，避免闪屏）
    fn stream_update_lyrics(
        &mut self,
        ui: &mut UiState,
        current_idx: usize,
    ) -> std::io::Result<()> {
        if let Some(lyrics) = &ui.lyrics {
            if lyrics.lines.is_empty() {
                return Ok(());
            }

            let start = current_idx.saturating_sub(3);
            let end = (current_idx + 4).min(lyrics.lines.len());

            // 如果范围没有变化且只是当前行的高亮变化，使用更精细的更新
            if let Some((last_start, last_end)) = ui.last_lyrics_range {
                if start == last_start && end == last_end {
                    return self.update_lyrics_highlight_only(ui, current_idx, start, end);
                }
            }

            // 初始化位置
            if ui.lyrics_base_row.is_none() {
                ui.lyrics_base_row = Some(10);
            }

            let base_row = ui.lyrics_base_row.unwrap();

            // 保存光标位置
            print!("\x1b7"); // 保存光标位置

            // 一次性构建所有更新内容，减少IO操作
            let mut buffer = String::with_capacity(1024);

            // 更新歌词区域
            for (line_offset, i) in (start..end).enumerate() {
                let row = base_row + line_offset as u16 + 1;
                let (_, ref text) = lyrics.lines[i];

                // 使用ANSI转义序列移动光标到指定位置
                buffer.push_str(&format!("\x1b[{};1H", row));

                if i == current_idx {
                    // 当前高亮行：绿色 + 箭头
                    buffer.push_str(&format!(
                        "\x1b[32m\x1b[1m  ▶ {:<width$}\x1b[0m",
                        text,
                        width = UI_WIDTH.saturating_sub(4)
                    ));
                } else {
                    // 普通行：灰色
                    buffer.push_str(&format!(
                        "\x1b[90m    {:<width$}\x1b[0m",
                        text,
                        width = UI_WIDTH.saturating_sub(4)
                    ));
                }
            }

            // 清理下方可能的剩余行
            for line_offset in (end - start)..7 {
                let row = base_row + line_offset as u16 + 1;
                buffer.push_str(&format!("\x1b[{};1H{:<width$}", row, "", width = UI_WIDTH));
            }

            // 一次性输出所有内容，然后恢复光标
            print!("{}", buffer);
            print!("\x1b8"); // 恢复光标位置

            // 更新记录的范围
            ui.last_lyrics_range = Some((start, end));

            // 刷新输出
            std::io::Write::flush(&mut std::io::stdout())?;
        }

        Ok(())
    }

    // 只更新高亮状态，不移动文本
    fn update_lyrics_highlight_only(
        &self,
        ui: &mut UiState,
        current_idx: usize,
        start: usize,
        end: usize,
    ) -> std::io::Result<()> {
        if let Some(lyrics) = &ui.lyrics {
            let base_row = ui.lyrics_base_row.unwrap();

            print!("\x1b7"); // 保存光标位置

            let mut buffer = String::with_capacity(512);

            // 只更新颜色，不移动文本
            for (line_offset, i) in (start..end).enumerate() {
                let row = base_row + line_offset as u16 + 1;
                let (_, ref text) = lyrics.lines[i];

                buffer.push_str(&format!("\x1b[{};1H", row));

                if i == current_idx {
                    // 当前高亮行
                    buffer.push_str(&format!(
                        "\x1b[32m\x1b[1m  ▶ {:<width$}\x1b[0m",
                        text,
                        width = UI_WIDTH.saturating_sub(4)
                    ));
                } else {
                    // 普通行
                    buffer.push_str(&format!(
                        "\x1b[90m    {:<width$}\x1b[0m",
                        text,
                        width = UI_WIDTH.saturating_sub(4)
                    ));
                }
            }

            print!("{}", buffer);
            print!("\x1b8"); // 恢复光标位置

            std::io::Write::flush(&mut std::io::stdout())?;
        }

        Ok(())
    }
    pub fn force_refresh_playing_interface(
        &self,
        ui: &mut UiState,
        pl: &PlaylistView,
    ) -> std::io::Result<()> {
        use crossterm::cursor::MoveTo;
        use crossterm::execute;
        use crossterm::terminal::{Clear, ClearType};

        let mut stdout = stdout();

        // 强制清屏并重新显示播放界面
        execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
        self.show_playing_interface(ui, pl)?;
        print!(">>： ");
        std::io::stdout().flush()?;

        Ok(())
    }
}
