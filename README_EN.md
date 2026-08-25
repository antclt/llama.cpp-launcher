<div align="center">

# llama.cpp-launcher

[![English](https://img.shields.io/badge/English-blue)](README_EN.md) [![中文](https://img.shields.io/badge/中文-red)](README.md)

</div>

❓ **What is this?**

A lightweight llama.cpp launcher built with Rust, providing a graphical interface to manage and run llama.cpp servers. Supports visual configuration, RPC mode, preset management, real-time logs, and multilingual UI.

📸 **Screenshots**

**Server Configuration Panel**
![Server Panel](screenshot/screenshot_server_en.png)

**Server Log Panel**
![Server Log Panel](screenshot/screenshot_server_log_en.png)

**Model Management Panel**
![Model Panel](screenshot/screenshot_model_en.png)

✨ **Features**

- 🖥️ Cross-platform GUI based on eframe/egui
- ⚙️ Visual configuration of llama.cpp server parameters (port, threads, GPU layers, etc.)
- 📁 File browser to select model files (.gguf format)
- 💾 Preset system — save and load frequently used configurations
- 📊 Real-time log display (with file logging support)
- 🌐 English and Chinese interface (auto-detect system language)
- 🚀 Auto-start on boot (Windows)
- 🔧 GPU layers management (Auto/All/Manual modes)
- 📱 RPC mode support (distributed inference)
- 🎯 Web UI integration
- 📊 Auto-calculate maximum VRAM available context
- 🔍 Colorful model file tag parsing (auto-identifies parameter count, quantization type, etc.)
- 📋 Launch command preview (read-only view of the final command)

🚀 **Installation**

### Build from Source

```bash
# Clone the repository
git clone https://github.com/yihuishou/llama.cpp-launcher.git
cd llama.cpp-launcher

# Build release version
cargo build --release

# Executable located at
# target/release/llama_cpp_launcher.exe
```

### Requirements

- Rust 1.70+
- Windows: Visual Studio Build Tools or MinGW
- Linux: X11 development libraries

⚡ **Quick Start**

1. Download [llama.cpp release](https://github.com/ggml-org/llama.cpp/releases) and extract
2. Place the launcher in the same directory as llama.cpp
3. Launch the application and select the Server tab
4. Click the Auto Detect button — the program will automatically find the llama.cpp server executable
5. Configure server parameters (port, threads, etc.)
6. Select the Model tab, click Auto Detect — the program will find `models` or `model` folders in the same directory and display model files
7. Select the model file to run (.gguf format)
8. Click the Start Server button
9. Once the Open Web Client button lights up, click to open the Web UI

📖 **Documentation**

- 📄 [Project Architecture](AGENTS.md) — learn about project structure and module design
- 🐛 [Issue Tracker](https://github.com/yihuishou/llama.cpp-launcher/issues)
- 💬 [Discussions](https://github.com/yihuishou/llama.cpp-launcher/discussions)

🤝 **Contributing**

1. Fork the project
2. Create a feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the branch (`git push origin feature/AmazingFeature`)
5. Create a Pull Request

### Development Setup

```bash
# Install dependencies
cargo build

# Run tests
cargo test

# Format code
cargo fmt

# Lint code
cargo clippy
```

📄 **License**

This project is licensed under the [MIT License](LICENSE) — see the LICENSE file for details.

---

**Project Structure:**
```
src/
├── main.rs          # App entry, font loading, log initialization
├── app.rs           # Main application logic, UI state management
├── config/          # Configuration management
│   └── settings.rs  # AppSettings, Preset, GpuLayersMode definitions and serialization
├── engine/          # Process management
│   ├── server.rs    # ServerManager - llama-server lifecycle management
│   └── rpc.rs       # RpcManager - rpc-server lifecycle management
├── ui/              # GUI panels (7 panels)
│   ├── server_panel.rs      # Server configuration panel
│   ├── rpc_panel.rs         # RPC configuration panel
│   ├── model_panel.rs       # Model selection panel (colorful tag parsing)
│   ├── params_panel.rs      # Parameter configuration panel
│   ├── log_panel.rs         # Real-time log panel
│   ├── launch_commands_panel.rs  # Launch command preview
│   └── presets_panel.rs     # Preset management panel
├── i18n.rs          # Internationalization (Chinese/English)
├── kv_cache.rs      # KV cache management
├── shortcut.rs      # Windows shortcut creation
└── spacing_debugger.rs  # UI debugging tool
```

**Tech Stack:**
- 🦀 Rust 2021
- 🖼️ eframe/egui 0.34 (GUI framework)
- 📦 serde/serde_json (configuration serialization)
- 📁 rfd (file dialog)
- 📊 egui-toast (notification component)
- 🔧 gguf-rs (GGUF model file handling)
- 🌍 sys-locale (system language detection)
- ⏱️ chrono (date/time handling)
- 📁 dirs (system directory access)
- 🔧 shortcuts-rs (Windows shortcut creation)

**Supported Platforms:**
- ✅ Windows 10/11
- ✅ Linux (X11)

**Related Links:**
- [llama.cpp](https://github.com/ggerganov/llama.cpp) — Core inference engine
- [egui](https://github.com/emilk/egui) — GUI framework
- [eframe](https://github.com/emilk/egui/tree/master/crates/eframe) — Native application framework
