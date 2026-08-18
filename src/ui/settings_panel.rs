//! 设置面板 —— 吸收原菜单栏的全部功能
//!
//! 分区：外观（主题色 8 色 + 深色模式）、语言、启动器（开机自启 / 桌面快捷方式 / 保存加载配置）、
//! 调试（保存日志文件 / 调试模式）、关于（版本 / 项目地址 / 关于弹窗）。

use crate::app::{disable_auto_start, enable_auto_start, open_repo_url};
use crate::config::settings::{AppSettings, SettingsManager};
use crate::i18n;
use crate::ui::widgets;
use egui::{Color32, RichText};

pub fn ui(
    ui: &mut egui::Ui,
    settings: &mut AppSettings,
    settings_manager: &SettingsManager,
    lang: &i18n::Language,
    show_about: &mut bool,
    debug_mode: &mut bool,
) {
    let accent = crate::theme::accent_color(&settings.accent_color);

    // 注意：不在这里重复渲染标题 "设置"——顶栏已经显示了当前页面标题

    // ── 外观 ──
    widgets::card(ui, i18n::t(i18n::Key::ThemeAppearance, lang), accent, |ui| {
        ui.label(i18n::t(i18n::Key::ThemeColor, lang));
        ui.horizontal_wrapped(|ui| {
            let colors = [
                "#0A84FF", "#FF3B30", "#FF9500", "#FFCC00", "#34C759", "#00C7BE", "#AF52DE",
                "#FF2D55",
            ];
            let cur = crate::theme::parse_hex(&settings.accent_color);
            for c in &colors {
                let rgb = crate::theme::parse_hex(c);
                let col = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                let selected = rgb == cur;
                // 判断是否需要深色勾（亮色块用深勾）
                let needs_dark_check = rgb[0] > 200 && rgb[1] > 200 && rgb[2] > 200;
                if widgets::color_swatch(ui, col, selected, !needs_dark_check).clicked() {
                    settings.accent_color = c.to_string();
                }
            }
        });
        ui.add_space(8.0);
        // ★ Toggle 新签名：开关在左，标签在右
        let theme_opts = [i18n::t(i18n::Key::ThemeLight, lang), i18n::t(i18n::Key::ThemeDark, lang), i18n::t(i18n::Key::ThemeSystem, lang)];
        let mut theme_idx = match settings.theme_mode.as_str() {
            "light" => 0,
            "dark" => 1,
            _ => 2,
        };
        widgets::segmented(ui, &theme_opts, &mut theme_idx, accent);
        settings.theme_mode = match theme_idx {
            0 => "light".to_string(),
            1 => "dark".to_string(),
            _ => "auto".to_string(),
        };
    });

    // ── 语言 ──
    widgets::card(ui, i18n::t(i18n::Key::LabelLanguage, lang), accent, |ui| {
        let zh = i18n::t(i18n::Key::LangZh, lang);
        let en = i18n::t(i18n::Key::LangEn, lang);
        let opts = [zh, en];
        let mut sel = match settings.language.as_str() {
            "en" => 1,
            _ => 0,
        };
        widgets::segmented(ui, &opts, &mut sel, accent);
        settings.language = if sel == 1 {
            "en".to_string()
        } else {
            "zh".to_string()
        };
    });

    // ── 启动器 ──
    widgets::card(ui, i18n::t(i18n::Key::SettingsLauncher, lang), accent, |ui| {
        let mut auto = settings.auto_start;
        // ★ Toggle 新签名：返回值直接用于判断是否变更
        if widgets::toggle(ui, &mut auto, i18n::t(i18n::Key::MenuItemAutoStart, lang), accent) {
            settings.auto_start = auto;
            if auto {
                enable_auto_start();
            } else {
                disable_auto_start();
            }
            let _ = settings_manager.save(settings);
        }

        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add(widgets::rounded_button(
                    i18n::t(i18n::Key::MenuItemSaveConfig, lang),
                    None,
                ))
                .clicked()
            {
                let _ = settings_manager.save(settings);
            }
            if ui
                .add(widgets::rounded_button(
                    i18n::t(i18n::Key::MenuItemLoadConfig, lang),
                    None,
                ))
                .clicked()
            {
                if let Ok(s) = settings_manager.load() {
                    *settings = s;
                }
            }
            if ui
                .add(widgets::rounded_button(
                    i18n::t(i18n::Key::MenuItemCreateShortcut, lang),
                    None,
                ))
                .clicked()
            {
                let _ = crate::shortcut::create_desktop_shortcut();
            }
        });
    });

    // ── 调试 ──
    widgets::card(ui, i18n::t(i18n::Key::MenuItemDebugMode, lang), accent, |ui| {
        let mut log_to_file = settings.log_to_file;
        // ★ Toggle 新签名
        if widgets::toggle(ui, &mut log_to_file, i18n::t(i18n::Key::MenuItemLogToFile, lang), accent) {
            crate::set_log_to_file(log_to_file);
            let _ = settings_manager.save(settings);
        }
        settings.log_to_file = log_to_file;

        widgets::toggle(ui, debug_mode, i18n::t(i18n::Key::MenuItemDebugMode, lang), accent);
    });

    // ── 关于 ──
    widgets::card(ui, i18n::t(i18n::Key::SettingsAbout, lang), accent, |ui| {
        ui.label(
            RichText::new(i18n::t(i18n::Key::AboutVersion, lang)).color(ui.visuals().text_color()),
        );
        ui.label(i18n::t(i18n::Key::AboutDescription, lang));
        ui.label(RichText::new(i18n::t(i18n::Key::AboutCopyright, lang)).small());
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if ui.add(widgets::rounded_button(i18n::t(i18n::Key::MenuItemRepo, lang), None)).clicked()
            {
                open_repo_url();
            }
            if ui.add(widgets::rounded_button(i18n::t(i18n::Key::AboutTitle, lang), None)).clicked()
            {
                *show_about = true;
            }
        });
    });
}
