use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Default, Clone)]
pub struct Lyrics {
    pub lines: Vec<(u128, String)>, // 毫秒时间戳 -> 歌词行
}

impl Lyrics {
    /// 解析同名 LRC 文件
    pub fn load_from_path(audio_path: &Path) -> Option<Self> {
        let mut lrc_path = audio_path.to_path_buf();
        lrc_path.set_extension("lrc");

        if !lrc_path.exists() {
            return None;
        }

        let file = File::open(lrc_path).ok()?;
        let reader = BufReader::new(file);
        let mut lines = vec![];

        for line in reader.lines() {
            if let Ok(l) = line {
                let mut parts = l.split(']').collect::<Vec<_>>();
                if parts.len() >= 2 {
                    if let Some(ts_str) = parts.get(0) {
                        let ts = ts_str.trim_start_matches('[');
                        if let Some(ms) = parse_timestamp(ts) {
                            let text = parts[1..].join("]").trim().to_string();
                            lines.push((ms, text));
                        }
                    }
                }
            }
        }

        lines.sort_by_key(|(ms, _)| *ms);

        Some(Lyrics { lines })
    }

    /// 根据毫秒时间返回当前行索引
    pub fn current_line_index(&self, millis: u128) -> usize {
        self.lines
            .iter()
            .enumerate()
            .rfind(|(_, (ts, _))| *ts <= millis)
            .map(|(idx, _)| idx)
            .unwrap_or(0)
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

fn parse_timestamp(ts: &str) -> Option<u128> {
    // 格式 mm:ss.xx 或 mm:ss.xxx
    let mut parts = ts.split(':');
    let mm = parts.next()?.parse::<u128>().ok()?;
    let ss_frac = parts.next()?;
    let mut ss_parts = ss_frac.split('.');
    let ss = ss_parts.next()?.parse::<u128>().ok()?;
    let frac = ss_parts.next().unwrap_or("0").parse::<u128>().ok()?;
    Some(mm * 60_000 + ss * 1000 + frac * 10) // LRC 两位毫秒
}
