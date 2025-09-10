use crate::playlist::PlaybackMode;

#[derive(Debug, Clone)]
pub enum Command {
    Help,
    Quit,
    Folder(String),
    List,
    Search(String),
    PlayIndex(usize),
    Pause,
    Resume,
    Next,
    Prev,
    Mode(PlaybackMode),
    Volume(u8),
    Lyrics,     // 切换歌词显示
    LyricsMode, // 切换歌词显示模式（流式 vs 清屏）
    Now,        // 显示当前播放信息
    Unknown(String),
}

pub fn parse_command(line: &str) -> Command {
    let t = line.trim();
    if !t.starts_with('/') {
        return Command::Unknown(t.to_string());
    }
    let mut parts = t[1..].split_whitespace();
    let cmd = parts.next().unwrap_or("");
    match cmd.to_lowercase().as_str() {
        "help" => Command::Help,
        "quit" | "exit" | "q" | "e" => Command::Quit,
        "folder" | "f" => {
            let rest = parts.collect::<Vec<_>>().join(" ");
            if rest.is_empty() {
                Command::Unknown(format!(
                    "/folder 命令需要指定路径参数，例如: /folder C:\\Music"
                ))
            } else {
                Command::Folder(rest)
            }
        }
        "list" | "ls" => Command::List,
        "search" => {
            let rest = parts.collect::<Vec<_>>().join(" ");
            if rest.is_empty() {
                Command::Unknown(format!(
                    "/search 命令需要指定搜索关键词，例如: /search 周杰伦"
                ))
            } else {
                Command::Search(rest)
            }
        }
        "play" => {
            if let Some(n) = parts.next() {
                if let Ok(idx1) = n.parse::<usize>() {
                    if idx1 == 0 {
                        return Command::Unknown(format!("歌曲序号从 1 开始，不能为 0"));
                    }
                    return Command::PlayIndex(idx1);
                }
                // 如果解析失败，返回未知命令
                return Command::Unknown(format!("无效的歌曲序号: {}，请输入数字", n));
            }
            // 没有参数时播放第一首歌曲
            Command::PlayIndex(1)
        }
        "pause" => Command::Pause,
        "resume" => Command::Resume,
        "next" => Command::Next,
        "prev" | "back" => Command::Prev,
        "mode" | "m" => match parts.next().unwrap_or("").to_lowercase().as_str() {
            "sequential" | "seq" => Command::Mode(PlaybackMode::Sequential),
            "repeatone" | "one" => Command::Mode(PlaybackMode::RepeatOne),
            "shuffle" | "shu" => Command::Mode(PlaybackMode::Shuffle),
            "" => Command::Unknown(format!(
                "/mode 命令需要指定模式参数: sequential(顺序), repeatone(单曲循环), shuffle(随机)"
            )),
            invalid => Command::Unknown(format!(
                "无效的播放模式: {}，支持: sequential, repeatone, shuffle",
                invalid
            )),
        },
        "volume" | "vol" => {
            if let Some(v) = parts.next() {
                if let Ok(mut vv) = v.parse::<i32>() {
                    if vv < 0 || vv > 100 {
                        return Command::Unknown(format!(
                            "音量值必须在 0-100 范围内，输入的值: {}",
                            vv
                        ));
                    }
                    vv = vv.clamp(0, 100);
                    return Command::Volume(vv as u8);
                } else {
                    return Command::Unknown(format!(
                        "无效的音量值: {}，请输入 0-100 之间的数字",
                        v
                    ));
                }
            }
            Command::Unknown(format!("/volume 命令需要指定音量值，例如: /volume 80"))
        }
        "lyrics" | "lrc" => Command::Lyrics,
        "lmode" | "lm" => Command::LyricsMode,
        "now" => Command::Now,
        _ => Command::Unknown(t.to_string()),
    }
}
