use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Default, Clone, Debug)]
pub struct Lyrics {
    pub lines: Vec<(u128, String)>, // 毫秒时间戳 -> 歌词行
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
}

impl Lyrics {
    /// 解析同名 LRC 文件
    pub fn load_from_path(audio_path: &Path) -> Option<Self> {
        let mut lrc_path = audio_path.to_path_buf();
        lrc_path.set_extension("lrc");

        if !lrc_path.exists() {
            return None;
        }

        let file = File::open(&lrc_path).ok()?;
        let reader = BufReader::new(file);
        let mut lines = vec![];
        let mut title = None;
        let mut artist = None;
        let mut album = None;

        for line_result in reader.lines() {
            let line = match line_result {
                Ok(l) => l,
                Err(_) => continue, // 跳过读取错误的行
            };

            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // 处理元数据标签
            if line.starts_with('[') && line.contains(']') {
                if let Some(end) = line.find(']') {
                    let tag_content = &line[1..end];
                    let text_content = line[end + 1..].trim();

                    // 尝试解析时间戳
                    if let Some(ms) = parse_timestamp(tag_content) {
                        if !text_content.is_empty() {
                            lines.push((ms, text_content.to_string()));
                        }
                    } else {
                        // 处理元数据标签
                        match tag_content.to_lowercase().as_str() {
                            s if s.starts_with("ti:") => {
                                title = Some(s[3..].trim().to_string());
                            }
                            s if s.starts_with("ar:") => {
                                artist = Some(s[3..].trim().to_string());
                            }
                            s if s.starts_with("al:") => {
                                album = Some(s[3..].trim().to_string());
                            }
                            _ => {} // 忽略其他标签
                        }
                    }
                }
            }
        }

        // 按时间顺序排序
        lines.sort_by_key(|(ms, _)| *ms);

        Some(Lyrics {
            lines,
            title,
            artist,
            album,
        })
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
    // 支持格式：mm:ss.xx, mm:ss.xxx, mm:ss, m:ss.xx 等
    let mut parts = ts.split(':');
    let mm = parts.next()?.parse::<u128>().ok()?;
    let ss_frac = parts.next()?;

    let mut ss_parts = ss_frac.split('.');
    let ss = ss_parts.next()?.parse::<u128>().ok()?;

    // 处理小数部分，支持不同长度
    let frac = if let Some(frac_str) = ss_parts.next() {
        let frac_num = frac_str.parse::<u128>().ok()?;
        match frac_str.len() {
            1 => frac_num * 100, // .x -> x00 毫秒
            2 => frac_num * 10,  // .xx -> xx0 毫秒
            3 => frac_num,       // .xxx -> xxx 毫秒
            _ => frac_num,       // 其他情况直接使用
        }
    } else {
        0 // 没有小数部分
    };

    Some(mm * 60_000 + ss * 1000 + frac)
}
