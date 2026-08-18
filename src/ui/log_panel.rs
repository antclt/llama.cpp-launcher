use crate::config::settings::AppSettings;
use crate::engine::server::{LogLevel, ServerManager};
use crate::i18n;
use crate::ui::widgets;

pub fn ui(
    ui: &mut egui::Ui,
    settings: &mut AppSettings,
    server: &mut ServerManager,
    lang: &i18n::Language,
) {
    let accent = crate::theme::accent_color(&settings.accent_color);
    widgets::card(ui, i18n::t(i18n::Key::PanelLogTitle, lang), accent, |ui| {
        let auto_scroll_before = settings.auto_scroll_logs;

        ui.horizontal(|ui| {
            if ui
                .small_button(i18n::t(i18n::Key::BtnClearLogs, lang))
                .clicked()
            {
                server.clear_logs();
            }
            widgets::toggle(ui, &mut settings.auto_scroll_logs, "", accent);
        });
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelMaxLogLines, lang));
            ui.add(egui::DragValue::new(&mut settings.max_log_lines).range(-1..=10000));
            ui.small(i18n::t(i18n::Key::HintLogSession, lang));
        });

        let should_scroll_to_bottom = !auto_scroll_before && settings.auto_scroll_logs;

        ui.add_space(4.0);

        let progress = server.progress();
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
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .id_salt("log_scroll_area")
                .stick_to_bottom(settings.auto_scroll_logs)
                .show(ui, |ui| {
                    if should_scroll_to_bottom {
                        ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                    }

                    let mut logs = server.logs();
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
