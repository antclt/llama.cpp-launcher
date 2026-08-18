use crate::config::settings::{is_server_binary_name, AppSettings, SettingsManager};
use crate::engine::server::ServerManager;
use crate::i18n;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

pub fn ui(
    ui: &mut egui::Ui,
    settings: &mut AppSettings,
    settings_manager: &SettingsManager,
    lang: &i18n::Language,
    #[cfg_attr(not(target_os = "linux"), allow(unused_variables))] server_manager: &ServerManager,
) {
    ui.heading(i18n::t(i18n::Key::PanelServerTitle, lang));
    ui.separator();

    // 二进制路径
    ui.horizontal(|ui| {
        ui.label(i18n::t(i18n::Key::LabelServerPath, lang));
        let mut server_path_str = settings.server_path.to_string_lossy().to_string();
        let response = ui.text_edit_singleline(&mut server_path_str);
        if response.changed() {
            settings.server_path = std::path::PathBuf::from(&server_path_str);
        }
    });

    // 按钮行
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

        // 自动授权按钮（仅 Linux 显示）
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
            if ui.add_enabled(server_exists, btn).clicked() {
                if let Err(e) = server_manager.authorize_server(&settings.server_path) {
                    log::error!("自动授权失败: {}", e);
                }
            }
        }

        // 查看 llama.cpp 版本按钮
        let server_path_valid = settings
            .server_path
            .file_name()
            .and_then(|f| f.to_str())
            .is_some_and(is_server_binary_name)
            && settings.server_path.exists();

        if ui
            .add_enabled(
                server_path_valid,
                egui::Button::new(i18n::t(i18n::Key::BtnCheckVersion, lang)),
            )
            .clicked()
        {
            // 使用 CREATE_NO_WINDOW 防止弹出命令行窗口（Windows）
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

        // 小字显示版本信息
        if !settings.llama_version.is_empty() {
            ui.small(egui::RichText::new(&settings.llama_version).weak());
        }
    });

    // 监听地址
    ui.horizontal(|ui| {
        ui.label(i18n::t(i18n::Key::LabelHost, lang));
        ui.text_edit_singleline(&mut settings.host);
        ui.label(i18n::t(i18n::Key::LabelPort, lang));
        ui.add(egui::DragValue::new(&mut settings.port).range(1..=65535));
    });

    // 快捷按钮
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

    // 并行槽位
    ui.horizontal(|ui| {
        ui.label(i18n::t(i18n::Key::LabelParallelSlots, lang));
        ui.add(egui::DragValue::new(&mut settings.parallel_slots).range(1..=1024));
    });

    // 模型别名
    ui.horizontal(|ui| {
        ui.label(i18n::t(i18n::Key::LabelAlias, lang));
        ui.text_edit_singleline(&mut settings.alias);
    });

    // 功能开关（统一紧凑排列）
    ui.checkbox(
        &mut settings.verbose,
        i18n::t(i18n::Key::CheckboxVerbose, lang),
    );

    // 原生日志时间戳
    ui.checkbox(
        &mut settings.log_timestamps,
        i18n::t(i18n::Key::CheckboxLogTimestamps, lang),
    );

    // 离线模式勾选框
    ui.checkbox(
        &mut settings.offline_mode,
        i18n::t(i18n::Key::CheckboxOfflineMode, lang),
    );

    ui.checkbox(
        &mut settings.rpc_mode,
        i18n::t(i18n::Key::CheckboxRpcMode, lang),
    );
    if settings.rpc_mode {
        ui.indent("rpc_endpoints", |ui| {
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelRpcEndpoints, lang));
                ui.text_edit_singleline(&mut settings.rpc_endpoints);
                ui.small(i18n::t(i18n::Key::HintRpcEndpoints, lang));
            });
        });
    }

    ui.checkbox(
        &mut settings.web_ui_enabled,
        i18n::t(i18n::Key::CheckboxEnableWebClient, lang),
    );
}
