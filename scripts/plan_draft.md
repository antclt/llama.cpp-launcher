# 分片 GGUF 文件支持计划

**创建时间**: 2026-08-30  
**版本**: 0.1.0  
**状态**: 草稿

---

## 一、概述

### 1.1 背景

llama.cpp 官方已支持分片 GGUF 文件（通过 --offload-layer 和分片合并机制），但本启动器目前仅支持单文件 GGUF。需要扩展以支持：
- 自动识别分片文件（model-00001-of-00003.gguf 等）
- 文件收集与完整性校验
- UI 显示优化
- 启动流程适配

### 1.2 目标

- [x] 自动识别并收集分片文件
- [x] 在模型面板中正确显示分片模型
- [x] 支持启动分片模型（合并后加载）
- [x] 提供分片状态指示器

---

## 二、架构设计

`
┌─────────────────────────────────────────────────────────────────┐
│                    Llama Launcher (扩展)                          │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────────┐    │
│  │   文件收集层 │  │   显示层     │  │    启动层            │    │
│  │ (File Gather)│  │ (UI Display) │  │  (Process Launcher) │    │
│  └─────────────┘  └──────────────┘  └──────────────────────┘    │
│         │                 │                    │                 │
│         └─────────────────┼────────────────────┘                 │
│                          ▼                                      │
│  ┌──────────────────────────────────────────────────────┐        │
│  │              分片管理器 (ShardManager)                 │        │
│  │  - 识别规则：model-*.gguf, gguf-*.part, model*.gguf    │        │
│  │  - 完整性校验：检查碎片数量、文件大小一致性             │        │
│  │  - 合并逻辑：调用 llama.cpp 的 --merge-shards 或合并文件    │        │
│  └──────────────────────────────────────────────────────┘        │
└─────────────────────────────────────────────────────────────────┘
`

---

## 三、详细修改方案

### 3.1 文件收集层 (File Gather)

#### 3.1.1 新增模块：src/shard.rs

**职责**：识别、收集、校验分片文件

`ust
/// 分片信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardInfo {
    pub base_name: String,          // 基础名称，如 "model-00001"
    pub shard_num: u32,             // 当前分片号，如 1
    pub total_shards: u32,          // 总分片数，如 3
    pub file_path: PathBuf,         // 分片文件路径
    pub file_size: u64,             // 分片大小
}

/// 分片模型信息（合并后）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardModel {
    pub base_name: String,          // 基础名称
    pub shard_count: u32,           // 分片数量
    pub shard_size: u64,            // 单片大小
    pub total_size: u64,            // 总大小
    pub shard_files: Vec<PathBuf>,  // 分片文件列表
    pub is_complete: bool,          // 是否完整
}

/// 分片管理器
pub struct ShardManager {
    pub(crate) base_dir: PathBuf,   // 模型目录
    pub(crate) pattern: GlobPattern, // 匹配模式
}

impl ShardManager {
    /// 扫描目录，返回所有分片模型信息
    pub fn scan(&self, base_dir: &Path) -> Vec<ShardModel> {
        // ... 实现
    }

    /// 判断一个文件是否为分片文件
    pub fn is_shard_file(&self, path: &Path) -> bool {
        // 规则 1: model-00001-of-00003.gguf
        // 规则 2: model-00001.gguf (无后缀)
        // 规则 3: gguf-00001-of-00003.part
        // 规则 4: gguf-model-00001-of-00003.gguf
        // ...
    }

    /// 从分片文件名解析信息
    fn parse_shard_info(&self, file_name: &str) -> Option<(u32, u32)> {
        // 解析文件名，提取 shard_num 和 total_shards
    }

    /// 校验分片完整性
    pub fn verify_completeness(&self, base_name: &str, shard_files: &[PathBuf]) -> bool {
        // ... 实现
    }
}
`

**核心逻辑**：
- 扫描 model 目录，匹配所有 .gguf 文件
- 对每个文件，判断是否为分片文件
- 按 ase_name + shard_num 分组
- 计算 	otal_shards，判断完整性

#### 3.1.2 修改：src/engine/server.rs

**职责**：在启动命令中添加分片合并参数

**修改点**：

`ust
// 在 ServerManager 中新增字段
pub struct ServerManager {
    // ...
    pub(crate) merge_shards: bool,  // 是否合并分片
    pub(crate) shard_base_name: Option<String>,
}

impl ServerManager {
    /// 构建启动命令（新增分片参数处理）
    pub fn build_command(
        &self,
        model_path: &Path,
        args: &AppSettings,
    ) -> Result<Vec<String>> {
        let mut cmd = vec!["llama-server".to_string()];
        cmd.push("--model".to_string());
        cmd.push(model_path.to_string_lossy().to_string());

        // 检查是否为分片文件
        if let Some(base_name) = self.shard_base_name.as_ref() {
            let total = self.shard_total_shards;
            let current = self.shard_current;

            // 方案 A: 使用 --merge-shards (如果 llama-server 支持)
            cmd.push("--merge-shards".to_string());
            cmd.push(base_name.to_string());

            // 方案 B: 手动合并（如果官方不支持）
            // 创建一个临时合并文件
            let merged_path = model_path.with_file_name(format!(
                "{}_merged.gguf",
                base_name
            ));
            cmd.push("--model".to_string());
            cmd.push(merged_path.to_string_lossy().to_string());

            // 添加合并参数（如果支持）
            cmd.push("--shard-count".to_string());
            cmd.push(total.to_string());
        }

        // ... 其他参数
        Ok(cmd)
    }
}
`

#### 3.1.3 修改：src/engine/rpc.rs

**职责**：RPC 启动时的分片处理

`ust
pub struct RpcManager {
    // ...
    pub(crate) merge_shards: bool,
    pub(crate) shard_base_name: Option<String>,
}

impl RpcManager {
    pub fn build_command(&self, model_path: &Path, args: &AppSettings) -> Result<Vec<String>> {
        // 与 ServerManager 类似的逻辑
        // ...
    }
}
`

---

### 3.2 显示层 (UI Display)

#### 3.2.1 修改：src/ui/model_panel.rs

**职责**：在文件列表中正确显示分片模型

**修改点**：

`ust
// 在 ModelPanel 中添加分片状态列
pub struct ModelFileItem {
    pub name: String,
    pub size: u64,
    pub shard_count: Option<u32>,    // 新增：分片数量
    pub shard_total: Option<u32>,    // 新增：总分片数
    pub is_shard: bool,              // 新增：是否为分片模型
    pub shard_progress: Option<f64>, // 新增：完整性进度 (0.0 ~ 1.0)
    pub status: ModelStatus,         // 新增：状态 (完整/不完整/未知)
}

// 在 render_file_list 中处理分片显示
fn render_file_list(&mut self, ctx: &Context, state: &mut ModelPanelState, items: &[ModelFileItem]) {
    for item in items {
        let mut row = ctx.add_sized_row(17.0);

        // ... 原有列 ...

        // 分片状态列
        if let Some(shard_count) = item.shard_count {
            let shard_status = if let Some(total) = item.shard_total {
                let progress = shard_count as f64 / total as f64;
                let label = if progress < 1.0 {
                    format!("{}/{}", shard_count, total)
                } else {
                    format!("{}", shard_count)
                };
                ctx.add_label(
                    egui::RichText::new(&label).color(egui::Color32::from_rgb(0, 210, 210)), // 青色
                );
            } else {
                ctx.add_label(egui::RichText::new("分片").color(egui::Color32::from_rgb(100, 150, 255))); // 蓝色
            };
        }
    }
}
`

**UI 效果示例**：

| 文件名 | 大小 | 参数量 | 分片 | 状态 |
|--------|------|--------|------|------|
| Qwen2.5-14B-Instruct.gguf | 24.5 GB | 14B | — | ✅ 完整 |
| model-00001-of-00003.gguf | 8.2 GB | — | 2/3 | 🟡 不完整 |
| model-00002-of-00003.gguf | 8.1 GB | — | 2/3 | 🟡 不完整 |
| model-00003-of-00003.gguf | 8.2 GB | — | 2/3 | 🟡 不完整 |

#### 3.2.2 修改：src/ui/helper.rs

**职责**：扩展模型信息解析，支持分片文件

`ust
/// 解析模型文件信息（扩展支持分片）
pub fn parse_model_info(
    file_name: &str,
    file_size: u64,
    shard_manager: &ShardManager,
) -> (String, u64, ShardInfo) {
    let (name, size, shard_info) = parse_shard_info(file_name, file_size, shard_manager);
    (name, size, shard_info)
}

/// 解析分片文件名
fn parse_shard_info(
    file_name: &str,
    file_size: u64,
    shard_manager: &ShardManager,
) -> (String, u64, ShardInfo) {
    // 匹配规则
    let (base_name, shard_num, total_shards) = match parse_filename(file_name) {
        Some(ParseResult { base, num, total }) => (base, num, total),
        None => return (file_name.to_string(), file_size, ShardInfo { ... }),
    };

    ShardInfo {
        base_name: base_name,
        shard_num,
        total_shards,
        file_path: path::path::PathBuf::from(file_name),
        file_size,
    }
}
`

#### 3.2.3 修改：src/ui/server_panel.rs

**职责**：启动分片模型时的状态显示

`ust
// 在 ServerStatus 中新增分片状态指示
pub struct ServerStatus {
    pub is_running: bool,
    pub model_name: String,
    pub shard_status: ShardStatus,  // 新增
}

pub enum ShardStatus {
    None,
    Detecting { current: u32, total: u32 },
    Merging { progress: f64 },
    Ready { shard_count: u32 },
}
`

---

### 3.3 启动层 (Process Launcher)

#### 3.3.1 修改：src/engine/server.rs

**职责**：处理分片合并与启动

**方案对比**：

| 方案 | 优点 | 缺点 | 优先级 |
|------|------|------|--------|
| A: 使用 --merge-shards | 简单、官方支持（如果可用） | 依赖 llama-server 版本 | ⭐⭐⭐ |
| B: 手动合并文件 | 通用、可控 | 需要额外文件操作 | ⭐⭐ |
| C: 动态追加参数 | 灵活 | 依赖 llama.cpp 支持 | ⭐ |

**推荐方案 B**（最通用）：

`ust
pub fn launch_shard_model(
    &self,
    shard_files: &[PathBuf],
    settings: &AppSettings,
) -> Result<Child> {
    // 1. 创建临时合并文件
    let merged_path = self.get_merged_path(&shard_files[0]);
    merge_shards(&shard_files, &merged_path)?;

    // 2. 启动服务器，使用合并后的文件
    let mut cmd = self.build_command(&merged_path, settings)?;

    // 3. 可选：如果 llama-server 支持 --merge-shards 参数
    cmd.push("--merge-shards".to_string());
    cmd.push(shard_files[0].file_stem().to_string_lossy().to_string());

    // 4. 启动进程
    self.launch(cmd)
}

/// 合并分片文件（使用 cat 或自定义二进制）
fn merge_shards(shards: &[PathBuf], output: &Path) -> Result<()> {
    use std::fs::File;
    use std::io::Write;

    let mut output_file = File::create(output)?;
    for shard in shards {
        let mut shard_file = File::open(shard)?;
        io::copy(&mut shard_file, &mut output_file)?;
    }
    Ok(())
}
`

#### 3.3.2 修改：src/engine/rpc.rs

**职责**：RPC 启动时的分片处理

`ust
impl RpcManager {
    pub fn launch_shard_model(
        &self,
        shard_files: &[PathBuf],
        settings: &AppSettings,
    ) -> Result<Child> {
        // 与 ServerManager 类似的逻辑
        // ...
    }
}
`

#### 3.3.3 修改：src/config/settings.rs

**职责**：增加分片相关配置项

`ust
/// 分片设置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShardSettings {
    /// 是否自动检测分片文件
    pub auto_detect_shards: bool,
    /// 分片合并策略
    ///   - "auto" : 自动选择最优策略
    ///   - "merge_file" : 手动合并文件
    ///   - "merge_param" : 使用 --merge-shards 参数
    pub merge_strategy: String,
    /// 分片合并输出目录
    pub merged_output_dir: Option<PathBuf>,
    /// 是否允许启动不完整的分片模型
    pub allow_incomplete_shards: bool,
}

/// AppSettings 添加
pub struct AppSettings {
    pub shard: ShardSettings,
    // ... 其他配置
}
`

---

### 3.4 配置层 (Configuration)

#### 3.4.1 新增：src/shard_settings.json

`json
{
  "auto_detect_shards": true,
  "merge_strategy": "auto",
  "merged_output_dir": "C:\\models\\merged",
  "allow_incomplete_shards": false
}
`

#### 3.4.2 修改：src/config/settings.rs

在 SettingsManager 中新增分片配置读写：

`ust
impl SettingsManager {
    pub fn load_shard_settings(&mut self) -> ShardSettings {
        // 从 JSON 加载或返回默认值
    }

    pub fn save_shard_settings(&self, settings: &ShardSettings) {
        // 保存分片配置
    }
}
`

---

## 四、依赖更新

### 4.1 新增依赖

`	oml
# Cargo.toml
[dependencies]
# 新增
glob = "0.3"
# 或
walkdir = "2.5"

# 可选：用于合并文件（如果不想依赖外部工具）
sha2 = "0.10"  # 用于完整性校验
`

### 4.2 依赖更新

`	oml
# 无需新增重大依赖
# 现有依赖已足够
`

---

## 五、文件清单

| 文件 | 操作 | 说明 |
|------|------|------|
| src/shard.rs | 新增 | 分片管理器核心逻辑 |
| src/shard_settings.json | 新增 | 分片配置 |
| src/engine/server.rs | 修改 | 启动命令添加分片参数 |
| src/engine/rpc.rs | 修改 | RPC 启动添加分片支持 |
| src/ui/model_panel.rs | 修改 | 文件列表显示分片状态 |
| src/ui/helper.rs | 修改 | 扩展模型信息解析 |
| src/ui/server_panel.rs | 修改 | 启动状态显示分片指示 |
| src/ui/rpc_panel.rs | 修改 | RPC 状态显示分片指示 |
| src/config/settings.rs | 修改 | 新增分片配置项 |
| Cargo.toml | 修改 | 新增 glob/walkdir 依赖 |

---

## 六、实施步骤

### 阶段 1：核心逻辑（2 天）
- [ ] 创建 src/shard.rs，实现 ShardManager
- [ ] 实现文件名解析规则
- [ ] 实现完整性校验逻辑

### 阶段 2：启动支持（1 天）
- [ ] 修改 ServerManager::build_command
- [ ] 修改 RpcManager::build_command
- [ ] 实现分片合并逻辑

### 阶段 3：UI 显示（1 天）
- [ ] 修改 ModelPanel 增加分片状态列
- [ ] 修改 ServerPanel 增加分片指示器
- [ ] 修改 RpcPanel 增加分片指示器

### 阶段 4：配置与测试（1 天）
- [ ] 实现配置保存/加载
- [ ] 单元测试：文件名解析
- [ ] 集成测试：完整流程
- [ ] 文档更新

---

## 七、风险与备选方案

### 7.1 风险

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| llama-server 不支持合并分片 | 中 | 高 | 使用方案 B：手动合并文件 |
| 分片文件命名不统一 | 中 | 中 | 支持多种命名模式 |
| 合并文件过大导致磁盘不足 | 低 | 中 | 提供临时目录设置 |

### 7.2 备选方案

- **方案 A**：如果 llama-server 支持 --merge-shards 参数，优先使用
- **方案 B**：手动合并文件到临时目录（推荐，最通用）
- **方案 C**：动态追加 --merge-shards 参数（依赖版本）

---

## 八、验收标准

- [ ] 能够正确识别分片文件（多种命名模式）
- [ ] 文件列表中显示分片数量（如 "2/3"）
- [ ] 能够成功启动分片模型
- [ ] 分片不完整时有明确提示
- [ ] 配置可持久化
- [ ] 单元测试覆盖 90% 以上

---

## 九、参考链接

- [llama.cpp 分片支持](https://github.com/ggerganov/llama.cpp)
- [GGUF 文件格式](https://github.com/ggerganov/llama.cpp/blob/master/gguf/gguf.h)

---

**备注**：本计划基于当前代码结构编写，实施时可根据实际依赖和 API 进行微调。
