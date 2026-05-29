# EyeCare - 护眼提醒助手

[English](#english) | [中文](#中文)

---

<a name="中文"></a>

## 中文

### 简介

EyeCare 是一款轻量级的护眼提醒桌面应用，基于 Tauri v2 开发。它通过智能监测您的**连续工作时长**，在适当的时候提醒您休息，帮助保护眼睛健康。支持 AI 智能生成个性化提醒文案，让休息提醒更加有趣和贴心。

**核心设计理念**：不同于传统定时器式提醒，EyeCare 基于连续工作时长触发提醒，并引入眼睛生命值系统、严重程度分级、自然休息检测和宠物养成等机制，让护眼成为一种健康习惯。

### 功能特性

#### 核心功能

- **智能休息提醒** - 基于连续工作时长触发提醒（默认 40 分钟），而非简单的定时器
- **眼睛生命值系统** - 直观显示眼睛疲劳程度（0-100），按工作时间衰减（约每 60 秒 -1），休息后恢复
- **自然休息检测** - 检测用户空闲状态（20 秒无操作），自动暂停计时；空闲超过 60 秒视为有效休息，重置工作计时器
- **严重程度分级** - 根据跳过次数和眼睛健康值，动态调整提醒语气（1-5 级）：
  - 1 级：温柔鼓励
  - 2 级：友好提醒
  - 3 级：关切严肃
  - 4 级：幽默讽刺
  - 5 级：黑色幽默 / 戏剧化
- **系统休眠感知** - 自动检测系统休眠/唤醒事件，暂停和恢复计时

#### 喝水提醒

- **上班排班模式** - 按办公时段智能提醒（6 个时段：9:00、10:30、11:30、13:30、15:00、17:00，约 1150ml/天）
- **固定间隔模式** - 自定义提醒间隔（10-120 分钟）、每日目标（500-4000ml）和每杯容量
- **进度追踪** - 实时显示今日饮水量、完成进度和排班时段状态
- **托盘快捷记录** - 右键菜单一键记录「已喝一杯水」

#### AI 智能增强

- **多服务商支持** - 硅基流动、OpenAI、DeepSeek、Ollama（本地部署），兼容所有 OpenAI 格式 API
- **个性化文案** - 根据工作时间、跳过次数、眼睛健康值、时间段、用户身份生成定制提醒
- **多种风格** - 温柔鼓励型、幽默俏皮型、暖心关怀型、科普知识型、平衡模式
- **本地消息兜底** - AI 不可用时自动使用 60+ 条本地消息库（按严重程度匹配）
- **智能缓存** - 相同上下文缓存 1 小时，减少重复 API 调用
- **请求限流** - 每 30 秒最多 1 次 API 调用，避免频繁请求
- **超时回退** - API 超时 8 秒自动回退预设文案
- **API 密钥加密** - 使用 AES-256-GCM 加密存储，基于机器标识派生密钥

#### 宠物系统

- **养成互动** - 休息完成增加宠物心情（+15），跳过休息降低心情（-20），抚摸互动（+3）
- **等级成长** - 累计休息次数提升宠物等级（公式：`1 + floor(sqrt(total_breaks))`，最高 99 级）
- **经验进度** - 每级所需经验为 `level²`，可视化升级进度
- **成就系统** - 解锁成就徽章（首次休息、10/50/100 次休息、5/10 级等）
- **心情衰减** - 持续工作 5 分钟宠物心情 -1
- **自定义命名** - 支持给宠物取名（最长 12 字符）

#### 系统集成

- **系统托盘** - 托盘图标显示倒计时和眼睛生命值，右键菜单快捷操作（暂停/休息/喝水/设置/退出）
- **应用白名单** - 在会议软件等应用中自动暂停计时（通过窗口标题检测前台应用）
- **开机自启** - 支持开机自动启动（支持 `--minimized` 参数静默启动）
- **全局快捷键** - `Ctrl+Shift+R` 立即休息、`Ctrl+Shift+P` 暂停/恢复监控
- **4K 背景图** - 全屏休息页面支持高清护眼背景图（网络加载 + 淡入动画）
- **便携模式** - 可执行文件同目录下放置 `.portable` 标记文件或 `config.json` 即可以便携模式运行

### 技术栈

| 层级 | 技术 | 说明 |
|------|------|------|
| **框架** | Tauri v2 | 轻量级跨平台桌面应用框架 |
| **前端** | HTML5 + CSS3 + Vanilla JavaScript (ES Module) | 无框架依赖，减少体积 |
| **后端** | Rust | 高性能、内存安全 |
| **存储** | JSON 配置文件 | 本地持久化，支持加密字段 |
| **HTTP** | reqwest + rustls-tls | HTTPS 加密传输 |
| **加密** | ring (AES-256-GCM) | API 密钥本地加密存储 |
| **空闲检测** | Win32 API / Cocoa / XScreenSaver | 跨平台原生空闲检测 |
| **打包** | NSIS / MSI | Windows 安装包 |

### 项目结构

```
eye-care/
├── src/                    # 前端资源
│   ├── index.html          # 主窗口（设置面板 + 状态展示）
│   ├── fullscreen.html     # 全屏休息页面（倒计时 + AI 文案 + 背景图）
│   ├── styles.css          # 全局样式
│   └── main.js             # 前端逻辑（Tauri IPC 通信）
├── src-tauri/              # Rust 后端
│   ├── src/
│   │   ├── main.rs         # 程序入口
│   │   ├── lib.rs          # 核心逻辑 + Tauri 命令注册
│   │   ├── idle.rs         # 空闲检测 + 监控循环 + 自然休息检测
│   │   ├── config.rs       # 配置管理（加载/保存/加密/便携模式）
│   │   ├── ai.rs           # AI 文案生成 + 本地消息库 + 缓存/限流
│   │   ├── tray.rs         # 系统托盘 + 全屏窗口管理
│   │   ├── water.rs        # 喝水提醒（排班模式 + 间隔模式）
│   │   └── crypto.rs       # AES-256-GCM 加密/解密
│   ├── capabilities/       # Tauri 权限配置
│   ├── icons/              # 应用图标
│   ├── Cargo.toml          # Rust 依赖
│   └── tauri.conf.json     # Tauri 应用配置
├── docs/                   # 文档
│   ├── SPEC.md             # 需求规格说明书
│   ├── MEDICATION_REMINDER.md  # 用药提醒功能方案
│   └── DEVELOPMENT.md      # 开发文档
├── package.json            # Node.js 依赖
└── LICENSE                 # Apache 2.0 License
```

### 安装

#### Windows

下载安装包：
- `EyeCare_1.0.0_x64-setup.exe` (NSIS 安装包，用户级安装，无需管理员权限)
- `EyeCare_1.0.0_x64_en-US.msi` (MSI 安装包)

#### 从源码构建

**前置要求**：
- [Node.js](https://nodejs.org/) (v16+)
- [Rust](https://www.rust-lang.org/tools/install) (v1.70+)
- Windows: Microsoft Visual Studio C++ Build Tools
- Linux: `libx11-dev` `libxss-dev` (apt) / `libX11-devel` `libXss-devel` (dnf)

```bash
# 克隆项目
git clone <repository-url>
cd eye-care

# 安装前端依赖
npm install

# 开发模式（热重载）
npm run dev

# 构建发布版本
npm run build
```

构建产物位于 `src-tauri/target/release/bundle/`。

### 配置

配置文件路径（按优先级）：
1. **环境变量**：`EYECARE_CONFIG_PATH`（指定文件路径）或 `EYECARE_CONFIG_DIR`（指定目录）
2. **便携模式**：可执行文件同目录下的 `config.json`（需存在 `.portable` 标记文件）
3. **系统默认**：
   - Windows: `%APPDATA%\eye-care\config.json`
   - macOS: `~/Library/Application Support/eye-care/config.json`
   - Linux: `~/.config/eye-care/config.json`

#### 配置示例

```json
{
  "version": "1.0.0",
  "idle_threshold": 40,
  "check_interval": 2,
  "break_duration": 60,
  "notify_before_fullscreen": 5,
  "max_skips_per_day": 3,
  "theme_color": "#E8F4F8",
  "auto_start": true,
  "ai": {
    "enabled": true,
    "provider": "siliconflow",
    "api_base_url": "https://api.siliconflow.cn/v1",
    "api_key": "enc:<AES-256-GCM encrypted>",
    "model": "Qwen/Qwen2.5-7B-Instruct",
    "preferred_style": "balanced"
  },
  "water": {
    "enabled": true,
    "schedule_enabled": true,
    "interval_minutes": 30,
    "daily_goal_ml": 2000,
    "cup_size_ml": 250
  }
}
```

### 默认设置

| 设置项 | 默认值 | 可配置范围 | 说明 |
|--------|--------|------------|------|
| 休息间隔 | 40 分钟 | 5-120 分钟 | 连续工作多久后提醒休息 |
| 休息时长 | 60 秒 | 15-120 秒 | 每次休息的倒计时时长 |
| 最大跳过次数 | 3 次/天 | 1-10 次 | 超过后进入强制休息模式 |
| 通知延迟 | 5 秒 | 0-30 秒 | 通知到全屏页面的等待时间 |
| 检测间隔 | 2 秒 | 1-10 秒 | 空闲状态轮询间隔 |
| 开机自启 | 开启 | - | 系统启动时自动运行 |
| 主题色 | #E8F4F8 | 3 种预设 | 淡蓝 / 米黄 / 浅绿 |

### 默认白名单应用

腾讯会议、Zoom、Teams、Skype、钉钉、飞书、腾讯QQ、微信、企业微信、Slack、Webex、GoTo Meeting、Google Meet

### 快捷键

| 快捷键 | 功能 |
|--------|------|
| `Ctrl+Shift+R` | 立即休息（全局） |
| `Ctrl+Shift+P` | 暂停/恢复监控（全局） |
| `Esc` | 关闭全屏休息页面 |

### 平台支持

| 平台 | 空闲检测 | 白名单 | 打包格式 |
|------|----------|--------|----------|
| Windows | GetLastInputInfo | 窗口标题检测 | NSIS / MSI |
| macOS | NSEvent | - | DMG (计划中) |
| Linux | XScreenSaver | - | AppImage (计划中) |

### 路线图

| 版本 | 功能 | 状态 |
|------|------|------|
| v0.2.0 | 护眼提醒 + 喝水提醒 + AI 文案 + 宠物系统 | 已完成 |
| v0.3.0 | **用药提醒** - 多药品管理、多级提醒、服药确认、库存预警 | 规划中 |
| v0.4.0 | **AI 用药助手** - 用药咨询对话、智能排程、依从性周报 | 规划中 |
| v0.5.0 | 药物相互作用检测、用药记录导出、多设备同步 | 远期 |

> 用药提醒功能详细方案见 [docs/MEDICATION_REMINDER.md](docs/MEDICATION_REMINDER.md)

### 许可证

[Apache License 2.0](LICENSE)

---

<a name="english"></a>

## English

### Introduction

EyeCare is a lightweight eye protection reminder desktop application built with Tauri v2. It intelligently monitors your **continuous work duration** and reminds you to take breaks at appropriate times to help protect your eye health. It supports AI-powered personalized reminder messages, making break reminders more engaging and caring.

**Core Design Philosophy**: Unlike traditional timer-based reminders, EyeCare triggers reminders based on continuous work duration, and introduces an eye health system, severity levels, natural break detection, and a pet companion to make eye care a healthy habit.

### Features

#### Core Features

- **Smart Break Reminders** - Triggered by continuous work duration (default 40 min), not simple timers
- **Eye Health System** - Visual display of eye fatigue (0-100), decays with work time (~1pt per 60s), recovers after breaks
- **Natural Break Detection** - Detects user idle state (20s threshold), automatically pauses timing; idle > 60s counts as effective break, resets work timer
- **Severity Levels (1-5)** - Dynamically adjusts reminder tone based on skip count and eye health:
  - Level 1: Gentle encouragement
  - Level 2: Friendly reminder
  - Level 3: Concerned/firm
  - Level 4: Sarcastic/humorous
  - Level 5: Dark humor/dramatic
- **System Sleep Awareness** - Auto-detects system sleep/wake events, pauses and resumes timing

#### Water Reminder

- **Work Schedule Mode** - Smart reminders across 6 office time slots (9:00, 10:30, 11:30, 13:30, 15:00, 17:00, ~1150ml/day)
- **Fixed Interval Mode** - Customizable reminder interval (10-120 min), daily goal (500-4000ml), and cup size
- **Progress Tracking** - Real-time display of daily water intake, completion progress, and schedule slot status
- **Tray Quick Record** - Right-click menu to quickly log a cup of water

#### AI Enhancement

- **Multiple Providers** - SiliconFlow, OpenAI, DeepSeek, Ollama (local), compatible with all OpenAI-format APIs
- **Context-Aware Messages** - Generates customized reminders based on work time, skip count, eye health, time of day, and user identity
- **Multiple Styles** - Gentle encouraging, humorous playful, caring, scientific, balanced mode
- **Local Message Fallback** - 60+ local messages with severity matching when AI is unavailable
- **Smart Caching** - 1-hour cache for identical contexts, reduces redundant API calls
- **Rate Limiting** - Max 1 API call per 30 seconds
- **Timeout Fallback** - 8-second API timeout with automatic fallback to preset messages
- **Encrypted API Keys** - AES-256-GCM encryption with machine-specific key derivation

#### Pet System

- **Interactive Companion** - Completing boosts mood (+15), skipping decreases mood (-20), petting interaction (+3)
- **Level Growth** - Accumulated breaks increase pet level (formula: `1 + floor(sqrt(total_breaks))`, max level 99)
- **XP Progress** - XP needed per level = `level²`, visual level-up progress bar
- **Achievement System** - Unlock badges (first break, 10/50/100 breaks, level 5/10, etc.)
- **Mood Decay** - Pet mood decreases by 1 every 5 minutes of continuous work
- **Custom Naming** - Name your pet (max 12 characters)

#### System Integration

- **System Tray** - Tray icon shows countdown and eye health, right-click menu for quick actions (pause/break/drink/settings/quit)
- **App Whitelist** - Automatically pause timing in meeting apps (detects foreground app via window title)
- **Auto Start** - Supports automatic startup on boot (supports `--minimized` for silent launch)
- **Global Shortcuts** - `Ctrl+Shift+R` for immediate break, `Ctrl+Shift+P` to pause/resume monitoring
- **4K Backgrounds** - Fullscreen break page supports HD background images (network loading + fade-in animation)
- **Portable Mode** - Place a `.portable` marker file or `config.json` alongside the executable for portable use

### Tech Stack

| Layer | Technology | Description |
|-------|-----------|-------------|
| **Framework** | Tauri v2 | Lightweight cross-platform desktop framework |
| **Frontend** | HTML5 + CSS3 + Vanilla JavaScript (ES Module) | Zero framework overhead |
| **Backend** | Rust | High performance, memory safety |
| **Storage** | JSON config file | Local persistence with encrypted fields |
| **HTTP** | reqwest + rustls-tls | HTTPS encrypted transport |
| **Encryption** | ring (AES-256-GCM) | Local API key encryption |
| **Idle Detection** | Win32 API / Cocoa / XScreenSaver | Native cross-platform idle detection |
| **Bundling** | NSIS / MSI | Windows installers |

### Project Structure

```
eye-care/
├── src/                    # Frontend assets
│   ├── index.html          # Main window (settings panel + status display)
│   ├── fullscreen.html     # Fullscreen break page (countdown + AI messages + backgrounds)
│   ├── styles.css          # Global styles
│   └── main.js             # Frontend logic (Tauri IPC communication)
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── main.rs         # Application entry point
│   │   ├── lib.rs          # Core logic + Tauri command registration
│   │   ├── idle.rs         # Idle detection + monitoring loop + natural break detection
│   │   ├── config.rs       # Config management (load/save/encrypt/portable mode)
│   │   ├── ai.rs           # AI message generation + local message library + cache/rate-limit
│   │   ├── tray.rs         # System tray + fullscreen window management
│   │   ├── water.rs        # Water reminders (schedule mode + interval mode)
│   │   └── crypto.rs       # AES-256-GCM encrypt/decrypt
│   ├── capabilities/       # Tauri permission configuration
│   ├── icons/              # Application icons
│   ├── Cargo.toml          # Rust dependencies
│   └── tauri.conf.json     # Tauri app configuration
├── docs/                   # Documentation
│   ├── SPEC.md             # Requirements specification
│   ├── MEDICATION_REMINDER.md  # Medication reminder feature design
│   └── DEVELOPMENT.md      # Development guide
├── package.json            # Node.js dependencies
└── LICENSE                 # Apache 2.0 License
```

### Installation

#### Windows

Download the installer:
- `EyeCare_1.0.0_x64-setup.exe` (NSIS installer, user-level install, no admin required)
- `EyeCare_1.0.0_x64_en-US.msi` (MSI installer)

#### Build from Source

**Prerequisites**:
- [Node.js](https://nodejs.org/) (v16+)
- [Rust](https://www.rust-lang.org/tools/install) (v1.70+)
- Windows: Microsoft Visual Studio C++ Build Tools
- Linux: `libx11-dev` `libxss-dev` (apt) / `libX11-devel` `libXss-devel` (dnf)

```bash
# Clone the repository
git clone <repository-url>
cd eye-care

# Install frontend dependencies
npm install

# Development mode (hot reload)
npm run dev

# Build for release
npm run build
```

Build output is located at `src-tauri/target/release/bundle/`.

### Configuration

Config file path (by priority):
1. **Environment Variable**: `EYECARE_CONFIG_PATH` (file path) or `EYECARE_CONFIG_DIR` (directory)
2. **Portable Mode**: `config.json` alongside executable (requires `.portable` marker file)
3. **System Default**:
   - Windows: `%APPDATA%\eye-care\config.json`
   - macOS: `~/Library/Application Support/eye-care/config.json`
   - Linux: `~/.config/eye-care/config.json`

#### Configuration Example

```json
{
  "version": "1.0.0",
  "idle_threshold": 40,
  "check_interval": 2,
  "break_duration": 60,
  "notify_before_fullscreen": 5,
  "max_skips_per_day": 3,
  "theme_color": "#E8F4F8",
  "auto_start": true,
  "ai": {
    "enabled": true,
    "provider": "siliconflow",
    "api_base_url": "https://api.siliconflow.cn/v1",
    "api_key": "enc:<AES-256-GCM encrypted>",
    "model": "Qwen/Qwen2.5-7B-Instruct",
    "preferred_style": "balanced"
  },
  "water": {
    "enabled": true,
    "schedule_enabled": true,
    "interval_minutes": 30,
    "daily_goal_ml": 2000,
    "cup_size_ml": 250
  }
}
```

### Default Settings

| Setting | Default | Range | Description |
|---------|---------|-------|-------------|
| Break Interval | 40 min | 5-120 min | Continuous work duration before break reminder |
| Break Duration | 60 sec | 15-120 sec | Countdown duration for each break |
| Max Skips | 3/day | 1-10 | Enters forced rest mode when exceeded |
| Notification Delay | 5 sec | 0-30 sec | Wait time from notification to fullscreen |
| Check Interval | 2 sec | 1-10 sec | Idle state polling interval |
| Auto Start | Enabled | - | Automatically run on system startup |
| Theme Color | #E8F4F8 | 3 presets | Light blue / Warm beige / Natural green |

### Default Whitelist Apps

Tencent Meeting, Zoom, Teams, Skype, DingTalk, Feishu, Tencent QQ, WeChat, Enterprise WeChat, Slack, Webex, GoTo Meeting, Google Meet

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+Shift+R` | Take a break now (global) |
| `Ctrl+Shift+P` | Pause/resume monitoring (global) |
| `Esc` | Close fullscreen break page |

### Platform Support

| Platform | Idle Detection | Whitelist | Bundle Format |
|----------|---------------|-----------|---------------|
| Windows | GetLastInputInfo | Window title detection | NSIS / MSI |
| macOS | NSEvent | - | DMG (planned) |
| Linux | XScreenSaver | - | AppImage (planned) |

### Roadmap

| Version | Feature | Status |
|---------|---------|--------|
| v0.2.0 | Eye break + Water reminder + AI messages + Pet system | Done |
| v0.3.0 | **Medication Reminder** - Multi-med management, escalation alerts, dose confirmation, stock alerts | Planned |
| v0.4.0 | **AI Medication Assistant** - Consultation dialog, smart scheduling, adherence reports | Planned |
| v0.5.0 | Drug interaction detection, medication log export, multi-device sync | Future |

> Detailed medication reminder design: [docs/MEDICATION_REMINDER.md](docs/MEDICATION_REMINDER.md)

### License

[Apache License 2.0](LICENSE)
