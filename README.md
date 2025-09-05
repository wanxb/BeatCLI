# 🎵 BeatCLI — Console Music Player

**BeatCLI** 是一个基于 Rust 的跨平台控制台音乐播放器，支持文件夹扫描、播放列表管理、播放模式、音量控制以及歌词显示（LRC 支持开发中）。

---

## 目录结构

```
beatcli/
├─ src/
│  ├─ command.rs       # 命令解析
│  ├─ lyrics.rs        # 歌词处理模块
│  ├─ player.rs        # 播放器实现
│  ├─ playlist.rs      # 播放列表管理
│  ├─ ui.rs            # 控制台 UI 与状态
│  └─ main.rs          # 程序入口
├─ Cargo.toml          # Rust 项目配置
└─ README.md
```

---

## 功能特性

- 扫描指定文件夹，自动添加支持的音频文件（MP3、FLAC 等）。
- 播放列表管理：顺序播放、单曲循环、随机播放。
- 控制台播放控制：

  - 播放 / 暂停 / 继续 / 上一首 / 下一首
  - 音量控制（0-100%）
  - 播放模式切换

- 歌词显示（LRC 支持开发中）
- 高度响应式控制台 UI，显示当前播放、下一首及播放状态。

---

## 安装

1. 安装 [Rust](https://www.rust-lang.org/tools/install)。
2. 克隆仓库：

```bash
git clone https://github.com/yourusername/beatcli.git
cd beatcli
```

3. 编译项目：

```bash
cargo build --release
```

4. 运行程序：

```bash
cargo run --release
```

---

## 控制台示例

启动后，你会看到欢迎界面：

```
========================================
     🎵 BeatCLI — Console Music Player
========================================

输入 /help 查看命令，/folder <path> 选择音乐目录
```

播放列表状态示例：

```
=================================================
当前播放: > 夜曲.flac
下一首: 夜的第七章.flac

播放模式: 顺序播放    音量: 80%    播放列表: 3 首
=================================================
  1. > 夜曲.flac
  2.   夜的第七章.flac
  3.   宝石gem-123.mp3
>>:
```

---

## 常用命令

| 命令                                      | 说明                                         |     |     |
| ----------------------------------------- | -------------------------------------------- | --- | --- |
| `/help`                                   | 显示帮助信息                                 |     |     |
| `/folder <path>`                          | 选择音乐文件夹，会扫描音频文件并加入播放列表 |     |     |
| `/list`                                   | 列出当前播放列表                             |     |     |
| `/play <N>`                               | 播放第 N 首歌曲（从 1 开始），默认播放第一首 |     |     |
| `/pause`                                  | 暂停播放                                     |     |     |
| `/resume`                                 | 继续播放                                     |     |     |
| `/next`                                   | 播放下一首                                   |     |     |
| `/prev`                                   | 播放上一首                                   |     |     |
| \`/mode \<Sequential/RepeatOne/Shuffle>\` | 切换播放模式                                 |     |     |
| `/volume <0..100>`                        | 设置音量百分比                               |     |     |
| `/quit`                                   | 退出程序                                     |     |     |

---

## 未来计划

- 完整 LRC 歌词解析和滚动显示。
- 播放暂停时歌词暂停。
- 播放列表保存与加载。
- 支持更多音频格式和控制功能。

---

## 快速上手示例

1. 添加音乐文件夹：

```
>>: /folder D:\Music
[Info] 扫描到 10 首歌曲
```

2. 播放第一首：

```
>>: /play 1
[OK] 开始播放: 夜曲.flac
```

3. 设置音量为 80%：

```
>>: /volume 80
[OK] 音量设置为: 80%
```

4. 切换到随机播放：

```
>>: /mode Shuffle
[OK] 已切换到随机播放模式
```

5. 查看播放列表：

```
>>: /list
播放列表:
  1. > 夜曲.flac
  2.   夜的第七章.flac
  3.   宝石gem-123.mp3
```

---

## 贡献

欢迎提交 [issue](https://github.com/yourusername/beatcli/issues) 或 [pull request](https://github.com/yourusername/beatcli/pulls)。

---

## 开源协议

本项目采用 MIT 协议开源。
