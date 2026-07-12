<div align="center">

<img src="docs/assets/logo.png" width="128" alt="Zenith logo">

# Zenith

**基于 Rust 与 Metal 的 macOS GPU 加速终端模拟器。**

[![Platform](https://img.shields.io/badge/platform-macOS-black?logo=apple)](https://github.com/gghhss2023/zenith/releases)
[![Rust](https://img.shields.io/badge/core-Rust-orange?logo=rust)](crates/)
[![Swift](https://img.shields.io/badge/UI-Swift%20%2B%20Metal-F05138?logo=swift)](Zenith/)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Follow on X](https://img.shields.io/badge/follow-%40qqqtelegram-1DA1F2?logo=x&logoColor=white)](https://x.com/qqqtelegram)

[English](README.md) · **简体中文**

*快在刀刃上，静在无声处。*

</div>

---

## 功能特性

| | |
|---|---|
| ⚡ **Metal 渲染** | 字形与背景全部 GPU 实例化渲染；按需重绘，静止画面零 CPU 占用 |
| 👻 **幽灵文字自动补全** | 基于命令历史的内联建议，按"频率 + 新近度"排序，`→` 或 `Tab` 一键接受 |
| 🤖 **AI 面板** | `⌘K` 呼出由 Claude 驱动的内联 AI 助手 |
| 🗂 **原生窗口与标签页** | 层叠式新窗口（`⌘N`）、macOS 原生标签页（`⌘T`），每个都是独立 shell 会话 |
| 🔎 **Shell 集成** | OSC 133 标记追踪提示符与命令边界，为上层功能提供数据 |
| 🖥 **全屏应用完美兼容** | `vim` / `less` / `btop` 等 alt-screen 应用进出无残留——光标、颜色、回滚全部正确恢复 |
| 🔠 **实时字号缩放** | `⌘+` / `⌘-` / `⌘0`，无需重启会话 |

## 安装

从 [Releases](https://github.com/gghhss2023/zenith/releases) 下载最新 `.dmg`，把 **Zenith** 拖进 `/Applications` 即可。

或从源码构建：

```bash
git clone https://github.com/gghhss2023/zenith.git
cd zenith
make install   # 编译 release 版本，打包 Zenith.app 并安装到 /Applications
```

**环境要求：** macOS 13+、Rust 工具链、Xcode Command Line Tools。

## 快捷键

| 快捷键 | 功能 |
|---|---|
| `⌘N` / `⌘T` | 新窗口 / 新标签页 |
| `⌘K` | 开关 AI 面板 |
| `→` 或 `Tab` | 接受内联建议 |
| `⌘+` `⌘-` `⌘0` | 调整 / 重置字号 |
| `⌘C` / `⌘V` / `⌘A` | 复制 / 粘贴 / 全选 |
| `⌃⌘F` | 全屏 |

## 架构

```
┌──────────────────────────────────────────────┐
│  Zenith.app (Swift + AppKit)                 │
│  窗口 · 标签页 · 输入 · 中文输入法 · AI 面板  │
├──────────────────────────────────────────────┤
│  Metal 渲染器                                 │
│  实例化字形/背景 · 字形图集                    │
├────────────────── C FFI ─────────────────────┤
│  zenith-core (Rust)                          │
│  VTE 解析 · 网格与回滚 · PTY                  │
│  OSC 133 shell 状态 · 历史频率排序             │
└──────────────────────────────────────────────┘
```

- **`crates/zenith-core`** — 终端状态机：网格、回滚、备用屏幕、shell 集成、历史记录
- **`crates/zenith-render`** — 字体光栅化、字形图集、GPU 实例生成
- **`crates/zenith-ffi`** — 供 Swift 调用的 C ABI 接口层
- **`Zenith/`** — SwiftPM 应用：AppKit 外壳、Metal 管线、NSTextInputClient（完整输入法支持）

## 开发

```bash
make build     # 调试构建（Rust + Swift）
make check     # cargo test + clippy
make app       # 打包 release .app 到 dist/
make dmg       # 生成可分发的磁盘镜像
```

## 许可证

[MIT](LICENSE)
