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
                Command::Unknown(t.to_string())
            } else {
                Command::Folder(rest)
            }
        }
        "list" | "ls" => Command::List,
        "search" => {
            let rest = parts.collect::<Vec<_>>().join(" ");
            if rest.is_empty() {
                Command::Unknown(t.to_string())
            } else {
                Command::Search(rest)
            }
        }
        "play" => {
            if let Some(n) = parts.next() {
                if let Ok(idx1) = n.parse::<usize>() {
                    return Command::PlayIndex(idx1.saturating_sub(1));
                }
            }
            Command::Unknown(t.to_string())
        }
        "pause" => Command::Pause,
        "resume" => Command::Resume,
        "next" => Command::Next,
        "prev" | "back" => Command::Prev,
        "mode" | "m" => match parts.next().unwrap_or("").to_lowercase().as_str() {
            "sequential" | "seq" => Command::Mode(PlaybackMode::Sequential),
            "repeatone" | "one" => Command::Mode(PlaybackMode::RepeatOne),
            "shuffle" | "shu" => Command::Mode(PlaybackMode::Shuffle),
            _ => Command::Unknown(t.to_string()),
        },
        "volume" | "vol" => {
            if let Some(v) = parts.next() {
                if let Ok(mut vv) = v.parse::<i32>() {
                    vv = vv.clamp(0, 100);
                    return Command::Volume(vv as u8);
                }
            }
            Command::Unknown(t.to_string())
        }
        _ => Command::Unknown(t.to_string()),
    }
}
