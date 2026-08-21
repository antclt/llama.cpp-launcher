use crate::config::settings::{
    is_rpc_binary_name, is_server_binary_name, AppSettings, SettingsManager,
};
use crate::downloader::DownloadHandle;
use crate::engine::rpc::{RpcManager, RpcState};
use crate::engine::server::{ServerManager, ServerState};
use crate::i18n::{self, Language};
use crate::spacing_debugger::SpacingDebugger;
use crate::ui::{
    launch_commands_panel, log_panel, model_panel, params_panel, presets_panel, rpc_panel,
    server_panel, settings_panel, widgets,
};
use egui::Color32;

/// 侧边栏导航分区（用枚举路由，避免依赖本地化字符串）
#[derive(Debug, Clone, Copy, PartialEq)]
enum NavSection {
    Server,
    Rpc,
    Model,
    Params,
    Log,
    RpcLog,
    Commands,
    Presets,
    Settings,
}

pub struct LlamaLauncherApp {
    settings: AppSettings,
    settings_manager: SettingsManager,
    server_manager: ServerManager,
    rpc_manager: RpcManager,
    downloader: DownloadHandle,
    nav: NavSection,
    logo: Option<egui::TextureHandle>,
    theme_applied: (bool, String),
    show_about: bool,
    lang: Language,
    auto_start_server_on_first_frame: bool, // 新增
    start_minimized: bool,                  // 开机自启时最小化到任务栏
    debug_mode: bool,                       // egui Inspector / 调试模式开关
    spacing_debugger: SpacingDebugger,      // UI 间距可视化工具
    title_bar_color_set: bool,              // 是否已设置过窗口标题栏颜色（仅一次）
    last_system_dark: Option<bool>,
}

impl LlamaLauncherApp {
    pub fn new(cc: &eframe::CreationContext<'_>, start_minimized: bool) -> Self {
        let settings_manager = SettingsManager::new();
        let mut settings = settings_manager.load().unwrap_or_default();

        // 应用自启动预设
        if let Some(ref preset_name) = settings.auto_start_preset_name {
            if let Some(preset) = settings.presets.iter().find(|p| p.name == *preset_name) {
                preset.clone().apply_to(&mut settings);
            }
        }

        let auto_start_server_on_first_frame = settings.auto_start_preset_name.is_some();

        // 语言：配置优先，其次按系统区域检测
        let locale = sys_locale::get_locale().unwrap_or_default();
        let lang = if !settings.language.is_empty() {
            if settings.language == "en" {
                Language::En
            } else {
                Language::Zh
            }
        } else if locale.starts_with("zh") {
            Language::Zh
        } else {
            Language::En
        };

        let server_manager = ServerManager::new();
        let rpc_manager = RpcManager::new();

        // 全局 UI 放大 1.3 倍（从 1.5 降到 1.3，用户要求更紧凑美观）
        cc.egui_ctx.set_zoom_factor(1.3);

        let mut last_system_dark = None;
        // 如果为跟随系统模式，启动时检测一次
        if settings.theme_mode == "auto" {
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                if let Ok(output) = std::process::Command::new("reg")
                    .args([
                        "query",
                        "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
                        "/v",
                        "AppsUseLightTheme",
                    ])
                    .creation_flags(0x0800_0000u32)
                    .output()
                {
                    if output.status.success() {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        for line in stdout.lines() {
                            if line.contains("AppsUseLightTheme") {
                                settings.dark_mode = line
                                    .split_whitespace()
                                    .last()
                                    .map_or(false, |v| v.contains("0x0"));
                                last_system_dark = Some(settings.dark_mode);
                            }
                        }
                    }
                }
            }
            if last_system_dark.is_none() {
                settings.dark_mode = false;
                last_system_dark = Some(false);
            }
        } else {
            settings.dark_mode = settings.theme_mode == "dark";
        }

        // 应用主题
        let accent = crate::theme::parse_hex(&settings.accent_color);
        crate::theme::apply_theme(&cc.egui_ctx, settings.dark_mode, accent);

        // 加载 llama 图标纹理
        let logo = load_logo_texture(&cc.egui_ctx);

        // 同步日志开关状态到全局标志
        crate::set_log_to_file(settings.log_to_file);

        Self {
            settings,
            settings_manager,
            server_manager,
            rpc_manager,
            downloader: DownloadHandle::new(),
            nav: NavSection::Server,
            logo,
            theme_applied: (true, String::new()), // 重新应用逻辑在 update 中处理
            show_about: false,
            lang,
            auto_start_server_on_first_frame,
            start_minimized,
            debug_mode: false,
            spacing_debugger: SpacingDebugger::new(),
            title_bar_color_set: false,
            last_system_dark,
        }
    }

    fn save(&mut self) {
        if let Err(e) = self.settings_manager.save(&self.settings) {
            log::error!("保存配置失败: {}", e);
        }
    }

    fn render_server_controls(&mut self, ui: &mut egui::Ui) {
        let server_state = self.server_manager.state();
        let start_fill = egui::Color32::from_rgb(40, 120, 40);
        let stop_fill = egui::Color32::from_rgb(180, 50, 50);
        match server_state {
            ServerState::Idle | ServerState::Error(_) => {
                let server_path_valid = self
                    .settings
                    .server_path
                    .file_name()
                    .and_then(|f| f.to_str())
                    .is_some_and(is_server_binary_name);
                let can_start =
                    server_path_valid && !self.settings.model_path.as_os_str().is_empty();
                let resp = ui.add_enabled(
                    can_start,
                    widgets::rounded_button(
                        i18n::t(i18n::Key::BtnStartServer, &self.lang),
                        Some(start_fill),
                    ),
                );
                if self.debug_mode {
                    self.spacing_debugger.rects.push(resp.rect);
                }
                if resp.clicked() {
                    self.server_manager.start(&self.settings);
                }
            }
            ServerState::Running => {
                let resp = ui.add(widgets::rounded_button(
                    i18n::t(i18n::Key::BtnStopServer, &self.lang),
                    Some(stop_fill),
                ));
                if self.debug_mode {
                    self.spacing_debugger.rects.push(resp.rect);
                }
                if resp.clicked() {
                    self.server_manager.stop();
                }
            }
            ServerState::Starting | ServerState::Stopping => {
                let resp = ui.label(i18n::t(i18n::Key::StatusProcessing, &self.lang));
                if self.debug_mode {
                    self.spacing_debugger.rects.push(resp.rect);
                }
            }
        }
    }

    fn render_rpc_controls(&mut self, ui: &mut egui::Ui) {
        let rpc_state = self.rpc_manager.state();
        let rpc_start_fill = egui::Color32::from_rgb(40, 100, 140);
        let rpc_stop_fill = egui::Color32::from_rgb(180, 50, 50);
        match rpc_state {
            RpcState::Idle | RpcState::Error(_) => {
                let rpc_path_valid = self
                    .settings
                    .rpc_server_path
                    .file_name()
                    .and_then(|f| f.to_str())
                    .is_some_and(is_rpc_binary_name);
                let resp = ui.add_enabled(
                    rpc_path_valid,
                    widgets::rounded_button(
                        i18n::t(i18n::Key::BtnStartRpc, &self.lang),
                        Some(rpc_start_fill),
                    ),
                );
                if self.debug_mode {
                    self.spacing_debugger.rects.push(resp.rect);
                }
                if resp.clicked() {
                    self.rpc_manager.start(&self.settings);
                }
            }
            RpcState::Running => {
                let resp = ui.add(widgets::rounded_button(
                    i18n::t(i18n::Key::BtnStopRpc, &self.lang),
                    Some(rpc_stop_fill),
                ));
                if self.debug_mode {
                    self.spacing_debugger.rects.push(resp.rect);
                }
                if resp.clicked() {
                    self.rpc_manager.stop();
                }
            }
            RpcState::Starting | RpcState::Stopping => {
                let resp = ui.label(i18n::t(i18n::Key::StatusProcessing, &self.lang));
                if self.debug_mode {
                    self.spacing_debugger.rects.push(resp.rect);
                }
            }
        }
    }

    fn render_web_client_button(&mut self, ui: &mut egui::Ui) {
        let accent = crate::theme::accent_color(&self.settings.accent_color);
        let web_accent =
            egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 175);
        // 可用条件：网页客户端开关开启 且 server 日志出现 "llama_server: listening on"
        let web_ready = self.settings.web_ui_enabled && self.server_manager.is_listening();
        let resp = ui.add_enabled(
            web_ready,
            widgets::rounded_button(
                i18n::t(i18n::Key::BtnOpenWebClient, &self.lang),
                Some(web_accent),
            ),
        );
        if self.debug_mode {
            self.spacing_debugger.rects.push(resp.rect);
        }
        if resp.clicked() {
            open_web_client_url(self.settings.port);
        }
    }

    fn render_sidebar(&mut self, ctx: &egui::Context) {
        // 侧边栏填充色 = panel_fill（与卡片同色），去掉默认分隔线
        let sidebar_fill = ctx.style().visuals.panel_fill;
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .show_separator_line(false)
            .default_width(198.0)
            .min_width(198.0)
            .max_width(198.0)
            .frame(
                egui::Frame::default()
                    .fill(sidebar_fill)
                    .stroke(egui::Stroke::NONE),
            )
            .show(ctx, |ui| {
                let accent = crate::theme::accent_color(&self.settings.accent_color);

                ui.add_space(10.0);
                // 品牌区：logo 放在带边框的圆角方框内（对齐 HTML .brand .logo）
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    let logo_box = egui::Frame::default()
                        .fill(ui.visuals().widgets.noninteractive.bg_fill)
                        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                        .corner_radius(8.0)
                        .inner_margin(egui::Margin::same(4));
                    let _ = logo_box.show(ui, |ui| {
                        if let Some(tex) = &self.logo {
                            ui.image((tex.id(), egui::vec2(28.0, 28.0)));
                        }
                    });
                    ui.add_space(0.0);
                    ui.vertical(|ui| {
                        // 注：不能用 ui.strong() —— 浅色模式下 strong 文字为白色会隐形
                        // 对齐 HTML 品牌：llama.cpp（大）+ Launcher（小）+ 版本号
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("llama.cpp")
                                    .color(ui.visuals().text_color())
                                    .size(14.0),
                            );
                            ui.label(
                                egui::RichText::new("Launcher")
                                    .color(ui.visuals().weak_text_color())
                                    .size(13.0)
                                    .strong(),
                            );
                        });
                        ui.label(
                            egui::RichText::new(concat!("v", env!("CARGO_PKG_VERSION")))
                                .color(ui.visuals().weak_text_color())
                                .size(10.0),
                        );
                    });
                });
                // HTML 侧栏无顶部分隔线（依赖 border-right），这里用间距代替
                ui.add_space(12.0);

                let items = [
                    (
                        NavSection::Server,
                        widgets::NavIcon::Server,
                        i18n::t(i18n::Key::NavServer, &self.lang),
                    ),
                    (
                        NavSection::Rpc,
                        widgets::NavIcon::Rpc,
                        i18n::t(i18n::Key::NavRpc, &self.lang),
                    ),
                    (
                        NavSection::Model,
                        widgets::NavIcon::Model,
                        i18n::t(i18n::Key::TabModel, &self.lang),
                    ),
                    (
                        NavSection::Params,
                        widgets::NavIcon::Params,
                        i18n::t(i18n::Key::TabParams, &self.lang),
                    ),
                    (
                        NavSection::Log,
                        widgets::NavIcon::Log,
                        i18n::t(i18n::Key::TabLog, &self.lang),
                    ),
                    (
                        NavSection::RpcLog,
                        widgets::NavIcon::RpcLog,
                        i18n::t(i18n::Key::TabRpcLog, &self.lang),
                    ),
                    (
                        NavSection::Commands,
                        widgets::NavIcon::Commands,
                        i18n::t(i18n::Key::TabCommands, &self.lang),
                    ),
                    (
                        NavSection::Presets,
                        widgets::NavIcon::Presets,
                        i18n::t(i18n::Key::TabPresets, &self.lang),
                    ),
                    (
                        NavSection::Settings,
                        widgets::NavIcon::Settings,
                        i18n::t(i18n::Key::NavSettings, &self.lang),
                    ),
                ];

                for &(section, icon, label) in &items {
                    let selected = self.nav == section;
                    if widgets::nav_row(ui, icon, label, selected, accent).clicked() {
                        self.nav = section;
                    }
                    ui.add_space(2.0); // 对齐 HTML .nav{gap:2px}
                }
            });
    }

    fn render_top_bar(&mut self, ctx: &egui::Context) {
        let title = match self.nav {
            NavSection::Server => i18n::t(i18n::Key::NavServer, &self.lang),
            NavSection::Rpc => i18n::t(i18n::Key::NavRpc, &self.lang),
            NavSection::Model => i18n::t(i18n::Key::TabModel, &self.lang),
            NavSection::Params => i18n::t(i18n::Key::TabParams, &self.lang),
            NavSection::Log => i18n::t(i18n::Key::TabLog, &self.lang),
            NavSection::RpcLog => i18n::t(i18n::Key::TabRpcLog, &self.lang),
            NavSection::Commands => i18n::t(i18n::Key::TabCommands, &self.lang),
            NavSection::Presets => i18n::t(i18n::Key::TabPresets, &self.lang),
            NavSection::Settings => i18n::t(i18n::Key::NavSettings, &self.lang),
        };
        let accent = crate::theme::accent_color(&self.settings.accent_color);

        egui::TopBottomPanel::top("top_panel")
            .show_separator_line(false) // 去掉顶栏底部分隔线
            .frame({
                let mut f = egui::Frame::default();
                f.inner_margin = egui::Margin {
                    top: 8_i8,
                    bottom: 6_i8,
                    left: 16_i8,
                    right: 16_i8,
                };
                // ★ 顶栏颜色统一为背景色（用户要求无色差）
                if self.settings.dark_mode {
                    f.fill = Color32::from_rgb(28, 28, 30); // #1C1C1E — 与主内容区同
                } else {
                    f.fill = Color32::from_rgb(245, 245, 247); // #F5F5F7 — 与主内容区同
                }
                f
            })
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // 标题放大（HTML .topbar .ttl{font-size:17px;font-weight:600}）
                    // ★ 不用 .strong()（浅色模式下 strong_text_color=白色→隐形），改用显式主文本色
                    ui.label(
                        egui::RichText::new(title)
                            .size(18.0)
                            .color(ui.visuals().text_color())
                            .strong(),
                    ); // strong 只影响字重（egui 0.33.3 实际不影响字重，但保留语义）

                    // 右侧分组：状态点 → 控制按钮 → 主题切换（RTL 布局实现右对齐）
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::theme_toggle_button(ui, self.settings.dark_mode, accent) {
                            self.settings.dark_mode = !self.settings.dark_mode;
                            self.settings.theme_mode = if self.settings.dark_mode {
                                "dark"
                            } else {
                                "light"
                            }
                            .to_string();
                        }
                        self.render_web_client_button(ui);
                        self.render_rpc_controls(ui);
                        self.render_server_controls(ui);

                        let rpc_running = self.rpc_manager.is_running();
                        let r_color = if rpc_running {
                            egui::Color32::from_rgb(110, 200, 255)
                        } else {
                            egui::Color32::GRAY
                        };
                        ui.label(i18n::t(i18n::Key::TabRpc, &self.lang)); // 放大（原 small）
                        widgets::status_dot(ui, r_color); // 尺寸在 widget 内放大

                        let server_running = self.server_manager.is_running();
                        let s_color = if server_running {
                            egui::Color32::from_rgb(110, 255, 140)
                        } else {
                            egui::Color32::GRAY
                        };
                        ui.label(i18n::t(i18n::Key::TabServer, &self.lang)); // 放大（原 small）
                        widgets::status_dot(ui, s_color); // 尺寸在 widget 内放大
                    });
                });
            });
    }
}

impl eframe::App for LlamaLauncherApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        ctx.request_repaint_after(std::time::Duration::from_millis(500));

        // 根据调试模式开关，控制 egui Inspector（悬浮时显示内置检查器面板）
        #[cfg(debug_assertions)]
        {
            ctx.set_debug_on_hover(self.debug_mode);
            ctx.style_mut(|s| {
                s.debug.hover_shows_next = self.debug_mode;
            });
        }

        // 调试模式：每帧开始时清空间距记录
        if self.debug_mode {
            self.spacing_debugger.begin_frame();
        }

        // 语言：配置优先，空字符串保持启动检测值
        self.lang = match self.settings.language.as_str() {
            "en" => Language::En,
            "zh" => Language::Zh,
            _ => self.lang,
        };

        // 跟随系统模式：使用启动时缓存的系统主题值，无需运行时检测
        if self.settings.theme_mode == "auto" {
            self.settings.dark_mode = self.last_system_dark.unwrap_or(false);
        } else {
            self.settings.dark_mode = self.settings.theme_mode == "dark";
        }

        // 主题变更检测：深色模式或主题色变化时重新应用
        let sig = (self.settings.dark_mode, self.settings.accent_color.clone());
        if sig != self.theme_applied {
            let accent = crate::theme::parse_hex(&self.settings.accent_color);
            crate::theme::apply_theme(ctx, self.settings.dark_mode, accent);
            self.theme_applied = sig;
            // 主题变化时同步更新原生标题栏颜色（与侧边栏/主内容区一致）
            set_title_bar_color(self.settings.dark_mode, frame);
        }

        // 应用启动时自动启动 Server
        if self.auto_start_server_on_first_frame {
            self.auto_start_server_on_first_frame = false;
            self.server_manager.start(&self.settings);
        }

        // ★ 首次帧：确保 Windows 标题栏颜色已设置（与侧边栏同色）
        // 注：主题块通常也会在首帧触发，这里作为保底，保证任意配置下都能着色
        if !self.title_bar_color_set {
            self.title_bar_color_set = true;
            set_title_bar_color(self.settings.dark_mode, frame);
        }

        // 开机自启时最小化到任务栏（仅第一帧执行）
        if self.start_minimized {
            self.start_minimized = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        self.server_manager.poll_logs();
        self.rpc_manager.poll();

        if self.show_about {
            egui::Window::new(i18n::t(i18n::Key::AboutTitle, &self.lang))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .fixed_size([150.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(i18n::t(i18n::Key::AboutVersion, &self.lang));
                    ui.label(i18n::t(i18n::Key::AboutDescription, &self.lang));
                    ui.separator();
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            egui::RichText::new(i18n::t(i18n::Key::AboutCopyright, &self.lang))
                                .size(10.0),
                        );
                    });
                    ui.horizontal_centered(|ui| {
                        if ui
                            .button(i18n::t(i18n::Key::BtnClose, &self.lang))
                            .clicked()
                        {
                            self.show_about = false;
                        }
                    });
                });
        }

        self.render_sidebar(ctx);
        self.render_top_bar(ctx);

        // ★ CentralPanel 显式设置 fill 为 window_fill（而非默认的 panel_fill）
        // 这样主内容区背景比侧边栏/卡片更深，形成 HTML 的三层层次：
        //   深色: content=#1C1C1E < sidebar+card=#2C2C2E
        //   浅色: content=#F5F5F7 < sidebar+card=#FFFFFF
        let content_fill = if self.settings.dark_mode {
            Color32::from_rgb(28, 28, 30) // #1C1C1E
        } else {
            Color32::from_rgb(245, 245, 247) // #F5F5F7
        };
        egui::CentralPanel::default()
            // 让内容区中的所有子框与内容边缘保留统一的呼吸空间。
            // 32px 与新版设计稿中卡片左右的留白一致。
            .frame(egui::Frame::default().fill(content_fill))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::Frame::default()
                        .inner_margin(egui::Margin::symmetric(32_i8, 0_i8))
                        .show(ui, |ui| match self.nav {
                            NavSection::Server => server_panel::ui(
                                ui,
                                &mut self.settings,
                                &self.settings_manager,
                                &self.lang,
                                &self.server_manager,
                                &self.downloader,
                            ),
                            NavSection::Rpc => rpc_panel::ui(
                                ui,
                                &mut self.settings,
                                &self.settings_manager,
                                &self.lang,
                                &self.rpc_manager,
                            ),
                            NavSection::Model => {
                                model_panel::ui(ui, &mut self.settings, &self.lang)
                            }
                            NavSection::Params => {
                                params_panel::ui(ui, &mut self.settings, &self.lang)
                            }
                            NavSection::Log => log_panel::ui(
                                ui,
                                &mut self.settings,
                                &mut self.server_manager,
                                &self.lang,
                            ),
                            NavSection::RpcLog => log_panel::rpc_ui(
                                ui,
                                &mut self.settings,
                                &mut self.rpc_manager,
                                &self.lang,
                            ),
                            NavSection::Commands => launch_commands_panel::ui(
                                ui,
                                &self.server_manager,
                                &self.rpc_manager,
                                &self.lang,
                                crate::theme::accent_color(&self.settings.accent_color),
                            ),
                            NavSection::Presets => {
                                let should_start =
                                    presets_panel::ui(ui, &mut self.settings, &self.lang);
                                if should_start {
                                    self.server_manager.start(&self.settings);
                                }
                            }
                            NavSection::Settings => settings_panel::ui(
                                ui,
                                &mut self.settings,
                                &self.settings_manager,
                                &self.lang,
                                &mut self.show_about,
                                &mut self.debug_mode,
                            ),
                        });
                });
            });

        // 调试模式：绘制控件间距可视化
        if self.debug_mode {
            self.spacing_debugger.visualize(ctx);
        }
    }
}

impl Drop for LlamaLauncherApp {
    fn drop(&mut self) {
        self.server_manager.stop();
        self.rpc_manager.stop();
        self.downloader.request_cancel();
        self.save();
    }
}

// 设置 Windows 原生标题栏（可拖动区域）颜色，使其与侧边栏/主内容区同色。
// 依赖 DWMWA_CAPTION_COLOR（Windows 11 22H2+）；旧版系统调用会失败并静默忽略，
// 标题栏回退为系统默认——符合需求中「不行就算了」的兜底预期。
#[cfg(target_os = "windows")]
fn set_title_bar_color(dark: bool, frame: &eframe::Frame) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_CAPTION_COLOR};

    // COLORREF 格式为 0x00BBGGRR（注意不是 RGB 顺序）
    let (r, g, b): (u8, u8, u8) = if dark {
        (0x2C, 0x2C, 0x2E) // #2C2C2E，与侧边栏一致
    } else {
        (0xFF, 0xFF, 0xFF) // #FFFFFF，与侧边栏一致
    };
    let colorref: u32 = ((b as u32) << 16) | ((g as u32) << 8) | (r as u32);

    let hwnd: HWND = match frame.window_handle() {
        Ok(h) => match h.as_raw() {
            RawWindowHandle::Win32(w) => w.hwnd.get() as HWND,
            _ => return, // 非 Win32 平台（如 wgpu/其它后端）直接跳过
        },
        Err(_) => return,
    };

    unsafe {
        // 忽略返回值：失败时（旧系统/无 DWM）标题栏保持系统默认即可
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_COLOR as u32,
            &colorref as *const u32 as *const std::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn set_title_bar_color(_dark: bool, _frame: &eframe::Frame) {}

/// 从 exe 同级及向上若干级目录查找 llama-cpp-launcher.png 并解码为纹理
fn load_logo_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let bytes = include_bytes!("../assets/llama.ico");
    let img = image::load_from_memory(bytes).ok()?;
    let img = img.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let pixels = img.into_raw();
    Some(ctx.load_texture(
        "llama_logo",
        egui::ColorImage::from_rgba_unmultiplied(size, &pixels),
        egui::TextureOptions::NEAREST,
    ))
}

// Windows 开机自启动注册表操作函数
#[cfg(target_os = "windows")]
pub(crate) fn enable_auto_start(silent: bool) {
    let exe_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            log::error!("获取当前 exe 路径失败: {}", e);
            return;
        }
    };

    let path_str = exe_path.to_string_lossy().to_string();
    let arg = if silent {
        format!("\"{}\" --minimized", path_str)
    } else {
        format!("\"{}\"", path_str)
    };

    match std::process::Command::new("reg")
        .arg("add")
        .arg(r#"HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run"#)
        .arg("/v")
        .arg("llama.cpp launcher")
        .arg("/d")
        .arg(arg)
        .arg("/f")
        .output()
    {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                log::error!("reg add 失败: {}", stderr.trim());
            } else {
                log::info!("开机自启注册表项已添加");
            }
        }
        Err(e) => {
            log::error!("执行 reg 命令出错: {}", e);
        }
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn disable_auto_start() {
    match std::process::Command::new("reg")
        .arg("delete")
        .arg(r#"HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run"#)
        .arg("/v")
        .arg("llama.cpp launcher")
        .arg("/f")
        .output()
    {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                log::error!("reg delete 失败: {}", stderr.trim());
            } else {
                log::info!("开机自启注册表项已移除");
            }
        }
        Err(e) => {
            log::error!("执行 reg 命令出错: {}", e);
        }
    }
}

// 非 Windows 平台的实现
#[cfg(not(target_os = "windows"))]
pub(crate) fn enable_auto_start(silent: bool) {
    use std::fs;

    let autostart_dir = dirs::config_dir()
        .map(|d| d.join("autostart"))
        .expect("无法获取 XDG config 目录");
    fs::create_dir_all(&autostart_dir).ok();

    let exe_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            log::error!("获取当前 exe 路径失败: {}", e);
            return;
        }
    };

    let exec_arg = if silent {
        format!("{} --minimized", exe_path.display())
    } else {
        format!("{}", exe_path.display())
    };

    let desktop_content = format!(
        r#"[Desktop Entry]
Type=Application
Name=LLama Launcher
Exec={}
Hidden=false
NoDisplay=false
X-GNOME-Autostart-enabled=true
"#,
        exec_arg
    );

    let desktop_path = autostart_dir.join("llama-cpp-launcher.desktop");
    match fs::write(&desktop_path, &desktop_content) {
        Ok(_) => log::info!("XDG autostart 文件已创建: {}", desktop_path.display()),
        Err(e) => log::error!("创建 autostart 文件失败: {}", e),
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn disable_auto_start() {
    use std::fs;

    let autostart_file = dirs::config_dir().map(|d| d.join("autostart/llama-cpp-launcher.desktop"));

    if let Some(path) = autostart_file {
        match fs::remove_file(&path) {
            Ok(_) => log::info!("XDG autostart 文件已删除: {}", path.display()),
            Err(e) => log::error!("删除 autostart 文件失败: {}", e),
        }
    }
}

// 用 ShellExecuteW 打开 URL，无黑窗口 (Windows)
#[cfg(target_os = "windows")]
mod shell_execute {
    use std::ffi::{c_void, OsStr};
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "shell32", kind = "dylib")]
    extern "system" {
        fn ShellExecuteW(
            hind_window: *mut c_void,
            lp_operation: *const u16,
            lp_file: *const u16,
            lp_parameters: *const u16,
            lp_directory: *const u16,
            n_show_cmd: i32,
        ) -> isize;
    }

    const SW_SHOW_NORMAL: i32 = 1;

    pub(crate) fn open_url(url: &str) {
        let op_utf16 = OsStr::new("open")
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<u16>>();

        let file_utf16 = OsStr::new(url)
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<u16>>();

        let _res = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                op_utf16.as_ptr(),
                file_utf16.as_ptr(),
                std::ptr::null::<u16>(),
                std::ptr::null::<u16>(),
                SW_SHOW_NORMAL,
            )
        };
    }
}

// WebClient: 用系统默认浏览器打开 http://127.0.0.1:<port>
#[cfg(target_os = "windows")]
fn open_web_client_url(port: u16) {
    let url = format!("http://127.0.0.1:{}", port);
    shell_execute::open_url(&url);
}

#[cfg(not(target_os = "windows"))]
fn open_web_client_url(port: u16) {
    use std::process::Command;
    let url = format!("http://127.0.0.1:{}", port);
    let _ = Command::new("xdg-open").arg(&url).spawn();
}

// GitHub 仓库页面
#[cfg(target_os = "windows")]
pub(crate) fn open_repo_url() {
    shell_execute::open_url("https://github.com/yihuishou/llama.cpp-launcher");
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn open_repo_url() {
    use std::process::Command;
    let url = "https://github.com/yihuishou/llama.cpp-launcher";
    let _ = Command::new("xdg-open").arg(url).spawn();
}
