use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::{
    fs::File,
    io::BufReader,
    path::Path,
    time::{Duration, Instant},
};

/// 播放器
pub struct Player {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sink: Option<Sink>,
    started_at: Option<Instant>,
    paused_at: Option<Instant>,
    elapsed_pause: Duration,
}

impl Player {
    pub fn new() -> anyhow::Result<Self> {
        let (_stream, handle) = OutputStream::try_default()?;
        Ok(Self {
            _stream,
            handle,
            sink: None,
            started_at: None,
            paused_at: None,
            elapsed_pause: Duration::ZERO,
        })
    }

    pub fn play_file(&mut self, path: &Path) {
        if let Some(s) = &self.sink {
            s.stop();
        }
        let file = match File::open(path) {
            Ok(f) => f,
            Err(_) => return,
        };
        let source = match Decoder::new(BufReader::new(file)) {
            Ok(s) => s,
            Err(_) => return,
        };
        let sink = Sink::try_new(&self.handle).expect("create sink");
        sink.append(source);

        sink.play();
        self.sink = Some(sink);
        self.started_at = Some(Instant::now());
        self.paused_at = None;
        self.elapsed_pause = Duration::ZERO;
    }

    pub fn pause(&mut self) {
        if let Some(s) = &self.sink {
            s.pause();
        }
        if self.paused_at.is_none() {
            self.paused_at = Some(Instant::now());
        }
    }

    pub fn resume(&mut self) {
        if let Some(s) = &self.sink {
            s.play();
        }
        if let Some(paused_time) = self.paused_at {
            self.elapsed_pause += paused_time.elapsed();
            self.paused_at = None;
        }
    }

    pub fn set_volume(&self, v: f32) {
        if let Some(s) = &self.sink {
            s.set_volume(v);
        }
    }

    pub fn finished(&self) -> bool {
        self.sink.as_ref().map(|s| s.empty()).unwrap_or(false)
    }

    pub fn get_current_ms(&self) -> u128 {
        if let Some(start) = self.started_at {
            let mut elapsed = start.elapsed();
            if let Some(paused) = self.paused_at {
                elapsed = paused.duration_since(start) - self.elapsed_pause;
            } else {
                elapsed -= self.elapsed_pause;
            }
            elapsed.as_millis()
        } else {
            0
        }
    }

    /// 停止播放并清理资源
    pub fn stop(&mut self) {
        if let Some(sink) = &self.sink {
            sink.stop();
        }
        self.sink = None;
        self.started_at = None;
        self.paused_at = None;
        self.elapsed_pause = Duration::ZERO;
    }
}
