use crate::config::settings::{AppSettings, SettingsManager};
use crate::engine::server::ServerManager;
use crate::i18n;
use crate::ui::widgets;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

pub fn ui(
    ui: &mut egui::Ui,
    settings: &mut AppSettings,
    settings_manager: &SettingsManager,
    lang: &i18n::Language,
    #[cfg_attr(not(target_os = "linux"), allow(unused_variables))] server_manager: &ServerManager,
) {
    let accent = crate::theme::accent_color(&settings.accent_color);

    // ── Server 路径 ──
    widgets::card(ui, i18n::t(i18n::Key::PanelServerTitle, lang), accent, |ui| {
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelServerPath, lang));
            let mut server_path_str = settings.server_path.to_string_lossy().to_string();
            let response = ui.text_edit_singleline(&mut server_path_str);
            if response.changed() {
                settings.server_path = std::path::PathBuf::from(&server_path_str);
            }
        });

        ui.horizontal(|ui| {
            if ui.button(i18n::t(i18n::Key::BtnBrowse, lang)).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title(i18n::t(i18n::Key::DialogSelectServer, lang))
                    .add_filter(i18n::t(i18n::Key::FilterExecutable, lang), &["exe"])
                    .pick_file()
                {
                    settings.server_path = path;
                }
            }
            if ui.button(i18n::t(i18n::Key::BtnAutoDetect, lang)).clicked() {
                if let Some(path) = settings_manager.auto_detect_server_path() {
                    settings.server_path = path;
                } else {
                    settings.server_path = std::path::PathBuf::from("");
                }
            }

            #[cfg(target_os = "linux")]
            {
                let server_exists = server_manager.check_server(&settings.server_path);
                let btn = egui::Button::new(i18n::t(i18n::Key::BtnAutoAuthorize, lang))
                    .min_size(egui::vec2(70.0, 20.0));
                let btn = if server_exists {
                    btn
                } else {
                    btn.sense(egui::Sense::hover())
                };
                if ui.add(btn).clicked() {
                    if let Err(e) = server_manager.authorize_server(&settings.server_path) {
                        log::error!("自动授权失败: {}", e);
                    }
                }
            }

            if ui
                .button(i18n::t(i18n::Key::BtnCheckVersion, lang))
                .clicked()
            {
                let mut cmd = std::process::Command::new(&settings.server_path);
                cmd.arg("--version")
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped());
                #[cfg(target_os = "windows")]
                cmd.creation_flags(0x0800_0000u32); // CREATE_NO_WINDOW
                match cmd.output() {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        let version = stdout
                            .lines()
                            .chain(stderr.lines())
                            .find(|line| line.contains("version:"))
                            .and_then(|line| {
                                line.split_once("version:")
                                    .map(|(_, v)| v.trim().to_string())
                            })
                            .unwrap_or_else(|| "未知版本".to_string());
                        settings.llama_version = version;
                    }
                    Err(e) => {
                        settings.llama_version = format!("获取失败: {}", e);
                    }
                }
            }

            if !settings.llama_version.is_empty() {
                ui.small(egui::RichText::new(&settings.llama_version).weak());
            }
        });
    });

    // ── 网络 ──
    widgets::card(ui, i18n::t(i18n::Key::SectionNetwork, lang), accent, |ui| {
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelHost, lang));
            ui.text_edit_singleline(&mut settings.host);
            ui.label(i18n::t(i18n::Key::LabelPort, lang));
            ui.add(egui::DragValue::new(&mut settings.port).range(1..=65535));
        });

        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new(i18n::t(i18n::Key::BtnHostLocal, lang))
                        .min_size(egui::vec2(40.0, 20.0)),
                )
                .clicked()
            {
                settings.host = "127.0.0.1".to_string();
            }
            if ui
                .add(
                    egui::Button::new(i18n::t(i18n::Key::BtnHostAny, lang))
                        .min_size(egui::vec2(50.0, 20.0)),
                )
                .clicked()
            {
                settings.host = "0.0.0.0".to_string();
            }
        });

        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelParallelSlots, lang));
            ui.add(egui::DragValue::new(&mut settings.parallel_slots).range(1..=32));
        });

        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelAlias, lang));
            ui.text_edit_singleline(&mut settings.alias);
        });
    });

    // ── 功能开关（Toggle 在左，标签在右）──
    widgets::card(ui, i18n::t(i18n::Key::SectionFeatures, lang), accent, |ui| {
        // ★ 新 toggle 签名：toggle(ui, &mut bool, "标签", accent) —— 内部渲染 [开关][标签]
        widgets::toggle(ui, &mut settings.verbose, i18n::t(i18n::Key::CheckboxVerbose, lang), accent);
        widgets::toggle(ui, &mut settings.log_timestamps, i18n::t(i18n::Key::CheckboxLogTimestamps, lang), accent);
        widgets::toggle(ui, &mut settings.offline_mode, i18n::t(i18n::Key::CheckboxOfflineMode, lang), accent);
        widgets::toggle(ui, &mut settings.rpc_mode, i18n::t(i18n::Key::CheckboxRpcMode, lang), accent);
        if settings.rpc_mode {
            ui.indent("rpc_endpoints", |ui| {
                ui.horizontal(|ui| {
                    ui.label(i18n::t(i18n::Key::LabelRpcEndpoints, lang));
                    ui.text_edit_singleline(&mut settings.rpc_endpoints);
                    ui.small(i18n::t(i18n::Key::HintRpcEndpoints, lang));
                });
            });
        }
        widgets::toggle(ui, &mut settings.web_ui_enabled, i18n::t(i18n::Key::CheckboxEnableWebClient, lang), accent);
    });
}
