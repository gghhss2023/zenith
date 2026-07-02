# Zenith 智能功能设计

日期：2026-07-02
状态：已确认（用户批准）

## 目标

为 Zenith（Rust core + Swift/Metal shell 的 macOS 终端模拟器）添加四个智能功能：

1. 自然语言转命令
2. 错误智能诊断
3. 输出总结/问答
4. 命令自动补全建议（本地，非 AI）

## 已确认的关键决策

| 决策 | 选择 | 理由 |
|------|------|------|
| AI 后端 | `claude` CLI 子进程（`claude -p`） | 走用户 Max 5x 订阅，零额外费用，无需管理 API key |
| 交互方式 | Cmd+K 悬浮面板 | 不污染终端内容；Zenith 未占用 Cmd+K，无冲突 |
| 施工顺序 | 两阶段：先 AI 面板，后本地补全 | 两套正交基础设施，风险隔离 |
| 安全规则 | AI 生成的命令只填入提示行，永不自动执行 | 用户回车是最后安检，防 LLM 出错与 prompt injection |
| 上下文范围 | 当前屏幕 + 最近 50 行滚动回看 | 够用、响应快、不过度泄露会话历史 |
| 面板实现 | 原生 AppKit（NSPanel + NSTextView） | 免去 Metal 自绘 UI 的文字排版/滚动/无障碍工作 |
| AI 输出 | stream-json + `--include-partial-messages` 流式逐字显示 | 避免 5-10 秒空等（已实测：无 partial 标志则无 text_delta） |
| 子进程隔离 | `--setting-sources "" --tools "" --strict-mcp-config` | 不加载用户 hooks/记忆/插件；禁用全部工具和 MCP，AI 无法自己执行命令（已实测 tools:[]）。禁用 `--bare`：会丢登录凭据 |
| 补全数据源 | OSC 133 shell 集成（非键入跟踪启发式） | 业界标准（Warp/iTerm2/kitty），精确知道命令起点 |
| 补全接受键 | `→`（光标在行尾时） | fish/zsh-autosuggestions 既有肌肉记忆 |

## Phase 1 — Cmd+K AI 面板

覆盖功能 1-3。三个模式共享同一条数据流：**用户输入 + 屏幕上下文 → prompt 模板 → `claude -p` → 流式显示结果**。

### 组件

**1. AIPanel（Swift, 新文件 `AIPanel.swift`）**
- NSPanel 悬浮于终端窗口，Cmd+K 呼出（`performKeyEquivalent` 路由），Esc 关闭
- 顶部 NSSegmentedControl 切换模式：生成命令 / 诊断错误 / 问答
- 中部 NSTextField 输入框（诊断错误模式可留空）
- 底部 NSTextView 只读结果区，流式追加
- 结果区下方按钮：「插入命令」（仅生成命令/诊断模式，将命令文本写入 PTY，不带 `\r`）

**2. 屏幕上下文捕获（Rust FFI）**
- 新增 `char *zn_terminal_screen_text(ZenithTerminal *term, uint32_t scrollback_lines)`
- 返回最近 N 行滚动回看 + 当前可见屏幕的纯文本（复用 grid 现有文本提取逻辑，行尾去空格）
- 用现有 `zn_string_free` 释放
- zenith.h 手工维护（cbindgen 已禁用），需同步声明

**3. ClaudeBridge（Swift, 新文件 `ClaudeBridge.swift`）**
- `Process` 启动（完整命令已实测验证，claude CLI v2.1.139）：
  `claude -p <prompt> --setting-sources "" --tools "" --strict-mcp-config --output-format stream-json --verbose --include-partial-messages`
- 后台队列逐行读 stdout，解析 JSONL：取 `stream_event` → `content_block_delta` → `text_delta` 增量文本，主线程回调更新面板；`result` 事件收尾（含 `is_error`）
- 跳过用户设置后默认模型为 sonnet（实测），对面板场景速度/质量合适；暂不暴露模型配置
- 60 秒超时终止进程；`claude` 不在 PATH 时面板内显示安装提示
- 每次请求独立进程，无会话保持（YAGNI）

### Prompt 模板（英文，嵌在代码中）

- **生成命令**：instruct 输出单条 macOS shell 命令、仅输出命令本身；附屏幕上下文 + 用户描述
- **诊断错误**：instruct 解释屏幕中最近的报错原因并给出修复命令；附屏幕上下文（+ 可选用户补充）
- **问答**：自由问题 + 屏幕上下文，正常回答

### 错误处理

- claude CLI 缺失 / 非零退出 / 超时 → 结果区显示错误信息，不崩溃
- 屏幕上下文为不可信输入：不做过滤，靠「命令永不自动执行」兜底

## Phase 2 — 本地历史补全

### OSC 133 shell 集成

- Rust 解析器新增 OSC 133 处理：`A`（提示符开始）、`B`（命令输入开始）、`C`（命令输出开始）、`D`（命令结束）；记录命令起点 (col,row)
- 提供 shell 集成脚本（bash: PROMPT_COMMAND；zsh: precmd/preexec），随 Zenith 分发；Zenith spawn shell 时设置 `ZENITH_SHELL_INTEGRATION` 环境变量指向脚本路径，用户在 rc 文件加一行 source（v1 不做自动注入）
- 附带红利：后续「诊断错误」可精确取"上一条命令 + 其输出"；为 Warp 式命令块铺路

### 历史存储与匹配

- 命令在 OSC 133 D（命令结束）时从 grid 提取并追加到 `~/.config/zenith/history`（权限 0600，命令行常含密钥）
- 内存中保留去重后的最近历史；当前输入 =「命令起点到光标」的 grid 文本，前缀匹配最近优先

### Ghost text 渲染

- 匹配命中的剩余部分以暗色（前景色 ~40% 亮度）glyph 绘制在光标之后
- 渲染层新增"建议文本"通道，不写入 grid（纯显示层）
- `→` 且光标位于已输入文本末尾 → 将剩余部分写入 PTY；其他任意键正常透传、建议即时刷新

## 测试

- Rust：`zn_terminal_screen_text` 提取逻辑单测；OSC 133 状态机单测（Phase 2）
- Swift：无测试基建，手动验证清单——Cmd+K 呼出/关闭、三模式流式返回、插入命令不自动执行、claude 缺失提示、中文输入不与 IME 冲突；Phase 2：ghost text 显示/接受/刷新、history 文件权限

## 非目标（本设计不做）

- 多 AI 后端抽象 / API 直连 / 本地模型
- Warp 式命令块 UI（OSC 133 只做数据层）
- AI 自动执行命令、Agent 模式
- 补全的模糊匹配/频率排序（v1 只做前缀 + 最近优先）
