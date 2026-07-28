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
<!-- 
| | |
| :---: | :---: |
| ![main menu](./assets/main_menu.png) | ![gameplay](./assets/gameplay.png) | -->

## 快速开始

### 项目配置

1. 从 [Releases 页面](https://github.com/Underline-1024/loom-engine/releases) 下载对应平台的打包文件，解压后运行可执行文件进入主菜单。

    ![main menu](./assets/main_menu.png)

2. 进入 Settings 页面，根据自身情况调整 Provider 和 Model 字段，完整设置介绍详见 [设置指南]()。

    ![settings](./assets/settings.png)

3. 配置环境变量。需要配置以下两个环境变量（将 `PROVIDER` 替换为实际使用的平台名称，如 `OPENAI`、`ANTHROPIC`）：
    -   `PROVIDER_BASE_URL`：模型 API 的 Base URL
    -   `PROVIDER_API_KEY`：对应平台的 API Key

### 使用流程

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

## 核心特性

-   **完整的数值系统与工具链**：内置数值属性、标签属性及背包系统，LLM 可通过全套工具函数实时读写游戏状态，而非仅生成文本。
-   **自然语言驱动交互**：玩家通过自然语言输入任意行动指令，LLM 自动解析意图、推进剧情走向，并同步更新底层数值与物品状态。
-   **广泛的模型兼容性**：原生支持 OpenAI、Anthropic、Ollama，以及任何兼容 OpenAI / Anthropic API 标准的本地或第三方模型平台。