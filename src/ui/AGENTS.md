# src/ui/ — UI 面板目录

## OVERVIEW

8 个 egui 面板 + 模型标签解析。纯渲染，业务逻辑委托给 app/config/engine。详见根 AGENTS.md / engine/AGENTS.md /
config/AGENTS.md。

## STRUCTURE

- server_panel: llama-server 路径/端口/槽位、启停/重启、状态、RPC 模式开关
- rpc_panel: ggml-rpc-server 路径/端口/threads/device/cache、启停、状态
- model_panel: GGUF 目录浏览、列表、彩色标签解析、mmproj/DFlash 切换
- params_panel: n_ctx/n_predict/temperature/top_p/top_k/repeat_penalty/kv_offload/cache_type/GPU
- log_panel: 服务器日志 ui() + 远程调用日志 rpc_ui()，共享 render()/LogSource trait
- launch_commands_panel: server/RPC 最终启动命令只读展示
- presets_panel: 预设保存/应用/删除/自启动，返回 bool(是否应启动 Server)

## WHERE TO LOOK

| Task               | Location                     | Notes                  |
|--------------------|------------------------------|------------------------|
| 面板函数签名与路由约定        | ui.rs / app.rs（tab_selected） | 路由由 app.rs 控制；本目录只负责渲染 |
| model_panel 标签解析规则 | model_panel::parse_tags()    | 按文件名分段着色               |

## CONVENTIONS

- 路由由 app.rs 按 tab_selected(i18n key) 控制；本目录不直接管理标签切换。
- 面板函数签名统一：fn ui(&mut Ui, settings: &mut AppSettings, lang: &Language)
- 例外（允许额外参数）:
    - model_panel: +&mut ServerManager, &mut RpcManager；FileMode(Main/Mmproj/DFlash)；auto_detect_model_dir /
      is_dflash_file()
    - log_panel: ui() +&mut ServerManager（服务器日志）；rpc_ui() +&mut RpcManager（远程调用日志）
    - presets_panel: bool 返回值用于触发 auto_start_server_on_first_frame

## ANTI-PATTERNS

- UI文本必须通过 i18n::t(Key, lang)，禁止硬编码。
- rfd: server/rpc面板 pick_file()，model面板 pick_folder()。
- 直接调用进程启动/停止；所有生命周期操作走 engine（ServerManager/RpcManager）。
