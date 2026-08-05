<!-- # Loom Engine

*一个由 LLM 驱动的叙事引擎与文字 RPG。* -->
<h1 align="center">Loom Engine</h1>

<p align="center">
  <em>一个由 LLM 驱动的叙事引擎与文字 RPG。</em>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-orange?logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/TUI-Ratatui-blue?logo=windowsterminal&logoColor=white" alt="TUI">
  <img src="https://img.shields.io/badge/LLM-Agent-green?logo=openai&logoColor=white" alt="LLM Agent">
</p>

<p align="center">
  <img src="./assets/main_menu.png" alt="main menu" width="48%">
  <img src="./assets/gameplay.png" alt="gameplay" width="48%">
</p>

<p align="center">
  <a href="./README.zh-CN.md">简体中文</a> | <a href="./README.md">English</a>
</p>

## 目录

-   [快速开始](#快速开始)
    -   [项目配置](#项目配置)
    -   [使用流程](#使用流程)
-   [本地开发与源码构建](#️-本地开发与源码构建)
-   [核心特性](#核心特性)
-   [未来规划](#-未来规划)
-   [设置指南](#设置指南)
-   [快捷键](#快捷键)
-   [许可证](#-许可证)

## 🚀 快速开始

> 如果你在使用这个项目的过程中遇到了问题或者对于这个项目有什么想法，欢迎进行反馈！你可以通过 [loom-engine@outlook.com](loom-engine@outlook.com) 这个邮箱来找我，也可以去 [Issues](https://github.com/Underline-1024/loom-engine/issues) 和 [Discussions](https://github.com/Underline-1024/loom-engine/discussions) 进行反馈和交流，我会尽力回复。

### ⚙️ 项目配置

1. 配置环境变量。需要配置以下两个环境变量（将 `PROVIDER` 替换为实际使用的平台名称，如 `OPENAI`、`ANTHROPIC`）：
    -   `PROVIDER_BASE_URL`：模型 API 的 Base URL
    -   `PROVIDER_API_KEY`：对应平台的 API Key

2. 从 [Releases 页面](https://github.com/Underline-1024/loom-engine/releases) 下载对应平台的打包文件，解压后运行可执行文件进入主菜单。

    ![main menu](./assets/main_menu.png)

3. 进入 Settings 页面，根据自身情况调整 Provider 和 Model 字段，完整设置介绍详见 [设置指南](#设置指南)。

    ![settings](./assets/settings.png)

### 🎮 使用流程

1. **主菜单**：使用 `↑` `↓` 选择选项，`Enter` 确认。

    ![main menu](./assets/main_menu.png)

2. **创建项目**：进入 Create 页面，使用 `Tab` 切换焦点。依次输入项目名称、选择项目模式、填写提示词（世界观设定或系统指令），最后切换至 CONFIRM 按钮并按 `Enter` 确认，等待项目初始化完成。

    ![create](./assets/create.png)

3. **选择项目**：项目创建完成后自动跳转至 Projects 页面，使用 `↑` `↓` 选择目标项目，`Enter` 进入。

    ![projects](./assets/projects.png)

4. **选择存档**：进入 Saves 页面，选择并加载一个存档。

    ![saves](./assets/saves.png)

5. **进入游戏**：进入 Gameplay 页面，界面包含属性面板、背包、对话历史及输入框。使用 `Tab` 切换面板焦点，`↑` `↓` 滚动列表或对话，`←` `→` 查看过长的文本条目；按 `Enter` 激活输入框，输入内容后再次按 `Enter` 发送，等待模型响应。

    ![gameplay](./assets/gameplay.png)

6. **保存**： 在你退出 Gameplay 页面之前，记得在输入框输入 `/save` 进行保存！你可以输入 `/help` 查看更多命令。

## 🛠️ 本地开发与源码构建

```bash
# 克隆仓库
git clone https://github.com/Underline-1024/loom-engine.git
cd loom-engine

# 编译并运行
cargo run
```

## ✨ 核心特性

-   **完整的数值系统与工具链**：内置数值属性、标签属性及背包系统，LLM 可通过全套工具函数实时读写游戏状态，而非仅生成文本。
-   **自然语言驱动交互**：玩家通过自然语言输入任意行动指令，LLM 自动解析意图、推进剧情走向，并同步更新底层数值与物品状态。
-   **广泛的模型兼容性**：原生支持 OpenAI、Anthropic、Ollama，以及任何兼容 OpenAI / Anthropic API 标准的本地或第三方模型平台。

## 🔮 未来规划

- [ ] 支持流式输出
- [ ] 支持插件扩展
- [ ] 引入本地向量数据库

## 📋 设置指南

>   *注意，设置完成后需保存设置并重启该应用才能生效*

| 字段 | 说明 |
| :--- | :--- |
| **Provider** | 模型平台，使用全小写 |
| **Enable Dynamic** | 是否启用动态特性，如启用则必须填写 `Embedding Model` 字段，启用则会开启动态工具调用，不启用则使用静态工具调用（某些平台不支持启用动态特性，当遇到不支持的平台时会自动忽略该字段） |
| **Model** | 模型名称 |
| **Embedding Model** | 嵌入模型名称，仅当启用动态特性时有效 |
| **Max Tokens** | 单次响应的最大 Token 限制 |
| **Max Turns** | 模型一次回答中的最大交互轮数（注意，不是最大对话轮数） |
| **System Prompt** | 全局系统指令 |

## ⌨️ 快捷键

| 快捷键 | 功能 | 作用范围 |
| :--- | :--- | :--- |
| `Enter` | 确认选择 / 激活输入框 / 发送指令 | 全局 |
| `↑` / `↓` | 上下滚动列表或对话历史 | 列表与对话面板 |
| `←` / `→` | 水平滚动查看过长的文本条目 | 当前高亮选中的列表项 |
| `Tab` | 切换面板焦点 (Stats / Tags / Inventory / Dialogue) | Gameplay 页面 |
| `Esc` | 返回上一级页面 / 退出程序 (在主菜单时) | 全局 |
| `Alt+Enter` / `Ctrl+O` | 换行 | Gameplay 页面 |

## 📄 许可证

本项目基于 MIT 许可证开源 - 详见 [LICENSE](./LICENSE) 文件。