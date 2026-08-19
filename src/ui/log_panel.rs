use crate::config::settings::AppSettings;
use crate::engine::rpc::RpcManager;
use crate::engine::server::{LogEntry, LogLevel, ServerManager};
use crate::i18n;
use crate::ui::widgets;

/// 日志源抽象：保证「服务器日志」与「远程调用日志」两个面板功能完全一致、永不漂移
trait LogSource {
    fn logs(&self) -> Vec<LogEntry>;
    fn clear_logs(&mut self);
}

impl LogSource for ServerManager {
    fn logs(&self) -> Vec<LogEntry> {
        ServerManager::logs(self)
    }
    fn clear_logs(&mut self) {
        ServerManager::clear_logs(self)
    }
}

impl LogSource for RpcManager {
    fn logs(&self) -> Vec<LogEntry> {
        RpcManager::logs(self)
    }
    fn clear_logs(&mut self) {
        RpcManager::clear_logs(self)
    }
}

fn render(
    ui: &mut egui::Ui,
    settings: &mut AppSettings,
    title_key: i18n::Key,
    lang: &i18n::Language,
    source: &mut dyn LogSource,
    progress: f32,
    scroll_salt: &'static str,
) {
    let accent = crate::theme::accent_color(&settings.accent_color);
    widgets::card(ui, i18n::t(title_key, lang), accent, |ui| {
        let auto_scroll_before = settings.auto_scroll_logs;

        ui.horizontal(|ui| {
            if ui
                .small_button(i18n::t(i18n::Key::BtnClearLogs, lang))
                .clicked()
            {
                source.clear_logs();
            }
            widgets::toggle(
                ui,
                &mut settings.auto_scroll_logs,
                i18n::t(i18n::Key::CheckboxAutoScroll, lang),
                accent,
            );
        });
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelMaxLogLines, lang));
            ui.add(egui::DragValue::new(&mut settings.max_log_lines).range(-1..=10000));
            ui.small(i18n::t(i18n::Key::HintLogSession, lang));
        });

        let should_scroll_to_bottom = !auto_scroll_before && settings.auto_scroll_logs;

        ui.add_space(4.0);

        if progress > 0.0 {
            let pct = (progress * 100.0).round() as u32;
            let label = format!(
                "{}: {}/100%",
                i18n::t(i18n::Key::LabelPreFillProgress, lang),
                pct
            );
            ui.add(egui::ProgressBar::new(progress).text(&label));
            ui.add_space(4.0);
        }

        if settings.max_log_lines != 0 {
            // 日志区撑满剩余高度（auto_shrink(false)），但留 8px 余量：
            // 否则 card 总高精确顶满外层 ScrollArea 的 inner 边界，
            // 任何微差都会触发外层垂直滚动条（同 presets 空态问题），
            // 且滚动条占宽后日志换行加高，锁存持续显示。
            let max_h = (ui.available_height() - 8.0).max(64.0);
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .max_height(max_h)
                .id_salt(scroll_salt)
                .stick_to_bottom(settings.auto_scroll_logs)
                .show(ui, |ui| {
                    if should_scroll_to_bottom {
                        ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                    }

                    let mut logs = source.logs();
                    if settings.max_log_lines > 0 && logs.len() > settings.max_log_lines as usize {
                        let start_index = logs.len() - settings.max_log_lines as usize;
                        logs.drain(..start_index);
                    }

                    let text_color = ui.visuals().text_color();
                    if logs.is_empty() {
                        ui.add_space(8.0);
                        ui.horizontal_centered(|ui| {
                            ui.colored_label(
                                egui::Color32::GRAY,
                                i18n::t(i18n::Key::HintNoLogs, lang),
                            );
                        });
                    } else {
                        for entry in &logs {
                            let prefix = match entry.level {
                                LogLevel::Info => "",
                                LogLevel::Warn => "⚠ ",
                                LogLevel::Error => "✖ ",
                            };
                            let text = format!("{}{}", prefix, entry.text);
                            ui.horizontal_wrapped(|ui| match entry.level {
                                LogLevel::Info => {
                                    ui.colored_label(text_color, &text);
                                }
                                LogLevel::Warn => {
                                    egui::Frame::default()
                                        .fill(egui::Color32::from_rgb(80, 80, 80))
                                        .inner_margin(egui::Margin::same(4))
                                        .corner_radius(6.0)
                                        .show(ui, |ui| {
                                            ui.colored_label(egui::Color32::YELLOW, &text);
                                        });
                                }
                                LogLevel::Error => {
                                    ui.colored_label(egui::Color32::RED, &text);
                                }
                            });
                        }
                    }
                });
        }
    });
}

/// 服务器日志面板（原「运行日志」，已改名）
pub fn ui(
    ui: &mut egui::Ui,
    settings: &mut AppSettings,
    server: &mut ServerManager,
    lang: &i18n::Language,
) {
    let progress = server.progress();
    render(
        ui,
        settings,
        i18n::Key::PanelLogTitle,
        lang,
        server,
        progress,
        "server_log_scroll_area",
    );
}

/// 远程调用日志面板（ggml-rpc-server 运行日志）；RPC 无 prefill 进度，传 0.0
pub fn rpc_ui(
    ui: &mut egui::Ui,
    settings: &mut AppSettings,
    rpc: &mut RpcManager,
    lang: &i18n::Language,
) {
    render(
        ui,
        settings,
        i18n::Key::PanelRpcLogTitle,
        lang,
        rpc,
        0.0,
        "rpc_log_scroll_area",
    );
}
