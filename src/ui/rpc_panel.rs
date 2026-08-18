use crate::config::settings::AppSettings;
use crate::engine::rpc::RpcManager;
use crate::i18n;
use crate::ui::widgets;

pub fn ui(
    ui: &mut egui::Ui,
    settings: &mut AppSettings,
    settings_manager: &crate::config::settings::SettingsManager,
    lang: &i18n::Language,
    #[cfg_attr(not(target_os = "linux"), allow(unused_variables))] rpc_manager: &RpcManager,
) {
    let accent = crate::theme::accent_color(&settings.accent_color);

    // ── RPC 路径 ──
    widgets::card(ui, i18n::t(i18n::Key::PanelRpcTitle, lang), accent, |ui| {
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelRpcPath, lang));
            let mut rpc_path_str = settings.rpc_server_path.to_string_lossy().to_string();
            let response = ui.text_edit_singleline(&mut rpc_path_str);
            if response.changed() {
                settings.rpc_server_path = std::path::PathBuf::from(&rpc_path_str);
            }
        });

        ui.horizontal(|ui| {
            if ui.button(i18n::t(i18n::Key::BtnBrowse, lang)).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title(i18n::t(i18n::Key::DialogSelectRpc, lang))
                    .add_filter(i18n::t(i18n::Key::FilterExecutable, lang), &["exe"])
                    .pick_file()
                {
                    settings.rpc_server_path = path;
                }
            }
            if ui.button(i18n::t(i18n::Key::BtnAutoDetect, lang)).clicked() {
                if let Some(path) = settings_manager.auto_detect_rpc_path() {
                    settings.rpc_server_path = path;
                } else {
                    settings.rpc_server_path = std::path::PathBuf::from("");
                }
            }

            #[cfg(target_os = "linux")]
            {
                let rpc_exists = rpc_manager.check_rpc_server(&settings.rpc_server_path);
                let btn = egui::Button::new(i18n::t(i18n::Key::BtnAutoAuthorize, lang))
                    .min_size(egui::vec2(70.0, 20.0));
                let btn = if rpc_exists {
                    btn
                } else {
                    btn.sense(egui::Sense::hover())
                };
                if ui.add(btn).clicked() {
                    if let Err(e) = rpc_manager.authorize_rpc_server(&settings.rpc_server_path) {
                        log::error!("自动授权失败: {}", e);
                    }
                }
            }
        });

        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelHost, lang));
            ui.text_edit_singleline(&mut settings.rpc_host);
            ui.label(i18n::t(i18n::Key::LabelPort, lang));
            ui.add(egui::DragValue::new(&mut settings.rpc_port).range(1..=65535));
        });

        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new(i18n::t(i18n::Key::BtnHostLocal, lang))
                        .min_size(egui::vec2(40.0, 20.0)),
                )
                .clicked()
            {
                settings.rpc_host = "127.0.0.1".to_string();
            }
            if ui
                .add(
                    egui::Button::new(i18n::t(i18n::Key::BtnHostAny, lang))
                        .min_size(egui::vec2(50.0, 20.0)),
                )
                .clicked()
            {
                settings.rpc_host = "0.0.0.0".to_string();
            }
        });

    // 线程数
    ui.horizontal(|ui| {
        ui.label(i18n::t(i18n::Key::LabelRpcThreads, lang));
        ui.add(egui::DragValue::new(&mut settings.rpc_threads).range(1..=1024));
        ui.small(i18n::t(i18n::Key::HintRpcThreads, lang));
    });

        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelRpcDevice, lang));
            ui.text_edit_singleline(&mut settings.rpc_device);
            ui.small(i18n::t(i18n::Key::HintRpcDevice, lang));
        });
    });

    // ── 缓存 ──
    widgets::card(ui, i18n::t(i18n::Key::SectionCache, lang), accent, |ui| {
        // ★ Toggle 新签名：开关在左，标签在右
        widgets::toggle(ui, &mut settings.rpc_cache, i18n::t(i18n::Key::CheckboxRpcCache, lang), accent);
    });
}
