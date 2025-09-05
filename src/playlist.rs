use rand::seq::SliceRandom;
use rand::thread_rng;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackMode {
    #[default]
    Sequential,
    RepeatOne,
    Shuffle,
}

#[derive(Default, Clone)]
pub struct Playlist {
    pub items: Vec<PathBuf>,
    pub current: Option<usize>,
    pub mode: PlaybackMode,
}

#[derive(Clone, Default)]
pub struct PlaylistView {
    pub len: usize,
    pub current: Option<usize>,
    pub mode: PlaybackMode,
    pub now_name: String,
    pub next_name: String,
}

impl Playlist {
    pub fn scan_folder(&mut self, folder: &str) -> anyhow::Result<usize> {
        self.items.clear();
        self.current = None;
        self.mode = PlaybackMode::Sequential;
        for entry in WalkDir::new(folder).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() && is_audio(path) {
                self.items.push(path.to_path_buf());
            }
        }
        Ok(self.items.len())
    }

    pub fn list(&self) -> Vec<(usize, std::path::PathBuf, bool)> {
        // 返回 (索引, 文件路径, 是否当前播放)
        self.items
            .iter()
            .enumerate()
            .map(|(i, p)| (i, p.clone(), Some(i) == self.current))
            .collect()
    }

    pub fn search(&self, q: &str) -> Vec<(usize, std::path::PathBuf)> {
        let ql = q.to_lowercase();
        self.items
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                let name = p.file_name().and_then(|s| s.to_str())?;
                if name.to_lowercase().contains(&ql) {
                    Some((i, p.clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn get(&self, idx: usize) -> Option<&PathBuf> {
        self.items.get(idx)
    }

    fn next_index_step(&self) -> Option<usize> {
        if self.items.is_empty() {
            return None;
        }
        match self.mode {
            PlaybackMode::Sequential => {
                let i = self.current.unwrap_or(0);
                Some((i + 1) % self.items.len())
            }
            PlaybackMode::RepeatOne => self.current,
            PlaybackMode::Shuffle => {
                let mut rng = thread_rng();
                let mut choices: Vec<usize> = (0..self.items.len()).collect();
                if let Some(cur) = self.current {
                    choices.retain(|&x| x != cur);
                }
                choices.choose(&mut rng).copied().or(self.current)
            }
        }
    }

    pub fn prev_index(&self) -> Option<usize> {
        if self.items.is_empty() {
            return None;
        }
        match self.mode {
            PlaybackMode::Sequential | PlaybackMode::RepeatOne => {
                let i = self.current.unwrap_or(0);
                Some(if i == 0 { self.items.len() - 1 } else { i - 1 })
            }
            PlaybackMode::Shuffle => self.next_index_step(),
        }
    }
    pub fn current_index(&self) -> Option<usize> {
        self.current
    }

    pub fn next_index(&mut self) -> Option<usize> {
        self.next_index_step()
    }

    /// 播放结束后，根据模式推进 current，并返回要播放的下标
    pub fn advance_on_finished(&mut self) -> Option<usize> {
        if self.items.is_empty() {
            return None;
        }
        match self.mode {
            PlaybackMode::Sequential => {
                let next = match self.current {
                    Some(i) => (i + 1) % self.items.len(),
                    None => 0,
                };
                self.current = Some(next);
                Some(next)
            }
            PlaybackMode::RepeatOne => self.current,
            PlaybackMode::Shuffle => {
                let mut rng = thread_rng();
                let mut choices: Vec<usize> = (0..self.items.len()).collect();
                if let Some(cur) = self.current {
                    choices.retain(|&x| x != cur);
                }
                let next = choices.choose(&mut rng).copied().or(self.current)?;
                self.current = Some(next);
                Some(next)
            }
        }
    }

    pub fn peek_next_name(&self) -> String {
        if self.items.is_empty() {
            return String::new();
        }
        let next = self.next_index_step();
        match next.and_then(|i| self.items.get(i)) {
            Some(p) => p
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string(),
            None => String::new(),
        }
    }

    pub fn clone_view(&self) -> PlaylistView {
        let now_name = match self.current.and_then(|i| self.items.get(i)) {
            Some(p) => p
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string(),
            None => String::new(),
        };
        let next_name = self.peek_next_name();
        PlaylistView {
            len: self.items.len(),
            current: self.current,
            mode: self.mode,
            now_name,
            next_name,
        }
    }
}

pub fn is_audio(path: &Path) -> bool {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
    {
        Some(ext) if matches!(ext.as_str(), "mp3" | "flac" | "wav" | "ogg" | "m4a" | "aac") => true,
        _ => false,
    }
}
