<div align="center">

# llama.cpp-launcher

[![English](https://img.shields.io/badge/English-blue)](README_EN.md) [![中文](https://img.shields.io/badge/中文-red)](README.md)

</div>

❓ **这是什么？**

这是一个用 Rust 构建的轻量级 llama.cpp 启动器，提供图形界面来管理和运行 llama.cpp 服务器。支持可视化配置、RPC模式、预设管理、实时日志和多语言界面。

📸 **屏幕截图**

**服务器配置面板**
![Server Panel](screenshot/screenshot_server.png)

**服务器日志面板**
![Server Log Panel](screenshot/screenshot_server_log.png)

**模型管理面板**
![Model Panel](screenshot/screenshot_model.png)

✨ **有什么特点？**

- 🖥️ 基于 eframe/egui 的跨平台 GUI 界面
- ⚙️ 可视化配置 llama.cpp 服务器参数（端口、线程、GPU 层数等）
- 📁 文件浏览器选择模型文件（支持 .gguf 格式）
- 💾 预设系统 - 保存和加载常用配置
- 📊 实时日志显示（支持文件日志记录）
- 🌐 支持中英文界面（自动检测系统语言）
- 🚀 开机自启支持（Windows）
- 🔧 GPU 层数管理（自动/全部/手动模式）
- 📱 支持自动计算显存可用最大上下文
- 📱 支持 RPC 模式（分布式推理）
- 🎯 支持 Web UI 集成
- 🔍 模型文件彩色标签解析（自动识别参数量、量化类型等）
- 📋 启动命令预览（只读展示最终命令）

🚀 **如何安装？**

### 从源码构建

```bash
# 克隆仓库
git clone https://github.com/yihuishou/llama.cpp-launcher.git
cd llama.cpp-launcher

# 构建发布版本
cargo build --release

# 可执行文件位于
# target/release/llama_cpp_launcher.exe
```

### 依赖要求

- Rust 1.70+
- Windows: 需要 Visual Studio Build Tools 或 MinGW
- Linux: 需要 X11 开发库

⚡ **快速使用？**

1. 下载 [llama.cpp 发布版](https://github.com/ggml-org/llama.cpp/releases) 并解压
2. 将启动程序放在 和 llama.cpp 同级目录
3. 启动应用程序，选择 Server 面板
4. 点击 自动检测 按钮，程序将自动查找 llama.cpp 服务器执行程序
5. 配置服务器参数（端口、线程数等）
6. 选择 模型管理 面板，点击 自动检测 按钮，程序将自动查找同级目录内的 models 或 model 文件夹并显示其中的模型文件
7. 选择要运行的模型文件（.gguf 格式）
8. 点击 启动Server 按钮
9. 待 打开网页客户端 按钮亮起，即可点击按钮打开 Web 界面

📖 **详细文档在哪里？**

- 📄 [项目架构文档](AGENTS.md) - 了解项目结构和模块设计
- 🐛 [问题反馈](https://github.com/yihuishou/llama.cpp-launcher/issues)
- 💬 [讨论区](https://github.com/yihuishou/llama.cpp-launcher/discussions)

🤝 **如何贡献？**

1. Fork 项目
2. 创建功能分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 创建 Pull Request

### 开发设置

```bash
# 安装依赖
cargo build

# 运行测试
cargo test

# 代码格式化
cargo fmt

# 代码检查
cargo clippy
```

📄 **用什么许可证？**

本项目采用 [MIT 许可证](LICENSE) - 查看 LICENSE 文件了解详情。

---

**项目结构：**
```
src/
├── main.rs          # 应用入口，字体加载，日志初始化
├── app.rs           # 主应用逻辑，UI 状态管理
├── config/          # 配置管理
│   └── settings.rs  # AppSettings, Preset, GpuLayersMode 定义与序列化
├── engine/          # 进程管理
│   ├── server.rs    # ServerManager - llama-server 生命周期管理
│   └── rpc.rs       # RpcManager - rpc-server 生命周期管理
├── ui/              # GUI 面板（7个面板）
│   ├── server_panel.rs      # 服务器配置面板
│   ├── rpc_panel.rs         # RPC 配置面板
│   ├── model_panel.rs       # 模型选择面板（彩色标签解析）
│   ├── params_panel.rs      # 参数配置面板
│   ├── log_panel.rs         # 实时日志面板
│   ├── launch_commands_panel.rs  # 启动命令预览
│   └── presets_panel.rs     # 预设管理面板
├── i18n.rs          # 国际化支持（中英文）
├── kv_cache.rs      # KV 缓存管理
├── shortcut.rs      # Windows 快捷方式创建
└── spacing_debugger.rs  # UI 调试工具
```

**技术栈：**
- 🦀 Rust 2021
- 🖼️ eframe/egui 0.34（GUI 框架）
- 📦 serde/serde_json（配置序列化）
- 📁 rfd（文件对话框）
- 📊 egui-toast（通知组件）
- 🔧 gguf-rs（GGUF 模型文件处理）
- 🌍 sys-locale（系统语言检测）
- ⏱️ chrono（时间日期处理）
- 📁 dirs（系统目录获取）
- 🔧 shortcuts-rs（Windows 快捷方式创建）

**支持的平台：**
- ✅ Windows 10/11
- ✅ Linux（X11）

**相关链接：**
- [llama.cpp](https://github.com/ggerganov/llama.cpp) - 核心推理引擎
- [egui](https://github.com/emilk/egui) - GUI 框架
- [eframe](https://github.com/emilk/egui/tree/master/crates/eframe) - 原生应用框架