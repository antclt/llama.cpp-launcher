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
    server_manager: &crate::engine::server::ServerManager,
    show_about: &mut bool,
    debug_mode: &mut bool,
    updater: &crate::updater::UpdaterHandle,
) {
    let accent = crate::theme::accent_color(&settings.accent_color);

    // 注意：不在这里重复渲染标题 "设置"——顶栏已经显示了当前页面标题

    // ── 外观 ──
    widgets::card(
        ui,
        i18n::t(i18n::Key::ThemeAppearance, lang),
        accent,
        |ui| {
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
            let theme_opts = [
                i18n::t(i18n::Key::ThemeLight, lang),
                i18n::t(i18n::Key::ThemeDark, lang),
                i18n::t(i18n::Key::ThemeSystem, lang),
            ];
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
        },
    );

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
    widgets::card(
        ui,
        i18n::t(i18n::Key::SettingsLauncher, lang),
        accent,
        |ui| {
            let mut auto = settings.auto_start;
            // ★ Toggle 新签名：返回值直接用于判断是否变更
            if widgets::toggle(
                ui,
                &mut auto,
                i18n::t(i18n::Key::MenuItemAutoStart, lang),
                accent,
            ) {
                settings.auto_start = auto;
                if auto {
                    enable_auto_start(settings.silent_start);
                } else {
                    disable_auto_start();
                }
                let _ = settings_manager.save(settings);
            }

            // 静默启动开关（仅在开机自启开启时显示）
            if settings.auto_start {
                ui.add_space(4.0);
                let mut silent = settings.silent_start;
                if widgets::toggle(
                    ui,
                    &mut silent,
                    i18n::t(i18n::Key::MenuItemSilentStart, lang),
                    accent,
                ) {
                    settings.silent_start = silent;
                    // 更新注册表项以反映新的静默启动设置
                    if settings.auto_start {
                        enable_auto_start(silent);
                    }
                    let _ = settings_manager.save(settings);
                }
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
        },
    );

    // ── 系统服务 ──
    widgets::card(
        ui,
        i18n::t(i18n::Key::SettingsSystemService, lang),
        accent,
        |ui| {
            ui.label(i18n::t(i18n::Key::SystemServiceDescription, lang));

            if true {
                // cfg!(target_os = "linux") - 暂时注释掉用于开发调试
                // Linux 平台：显示生成按钮
                if ui
                    .add(widgets::rounded_button(
                        i18n::t(i18n::Key::SystemServiceGenerate, lang),
                        None,
                    ))
                    .clicked()
                {
                    // 生成 systemd 服务文件
                    let template = i18n::t(i18n::Key::LinuxServiceFileContent, lang);
                    let cmd = server_manager.build_launch_command(settings);
                    let content = build_systemd_service_file(&template, &cmd);

                    let mut content = content;
                    ui.add_space(8.0);
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut content)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY),
                            );
                        });

                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add(widgets::rounded_button(
                                i18n::t(i18n::Key::BtnCopyServiceFile, lang),
                                None,
                            ))
                            .clicked()
                        {
                            ui.ctx().copy_text(content.to_string());
                        }
                        if ui
                            .add(widgets::rounded_button(
                                i18n::t(i18n::Key::BtnSaveServiceFile, lang),
                                None,
                            ))
                            .clicked()
                        {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_file_name("llama-server.service")
                                .save_file()
                            {
                                let path_str = path.to_string_lossy().to_string();
                                let mut f = std::fs::File::create(&path_str)
                                    .expect("Failed to create service file");
                                use std::io::Write;
                                f.write_all(content.as_bytes())
                                    .expect("Failed to write service file");
                            }
                        }
                    });
                }
            } else {
                // 非 Linux 平台：显示不可用提示
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(i18n::t(i18n::Key::SystemServiceNotAvailable, lang))
                        .color(egui::Color32::GRAY)
                        .italics(),
                );
            }
        },
    );

    // ── 调试 ──

    // ── 调试 ──
    widgets::card(
        ui,
        i18n::t(i18n::Key::MenuItemDebugMode, lang),
        accent,
        |ui| {
            let mut log_to_file = settings.log_to_file;
            // ★ Toggle 新签名
            if widgets::toggle(
                ui,
                &mut log_to_file,
                i18n::t(i18n::Key::MenuItemLogToFile, lang),
                accent,
            ) {
                crate::set_log_to_file(log_to_file);
                let _ = settings_manager.save(settings);
            }
            settings.log_to_file = log_to_file;

            widgets::toggle(
                ui,
                debug_mode,
                i18n::t(i18n::Key::MenuItemDebugMode, lang),
                accent,
            );
        },
    );

    // ── 关于 ──
    widgets::card(ui, i18n::t(i18n::Key::SettingsAbout, lang), accent, |ui| {
        ui.label(
            RichText::new(i18n::t(i18n::Key::AboutVersion, lang)).color(ui.visuals().text_color()),
        );
        ui.label(i18n::t(i18n::Key::AboutDescription, lang));
        ui.label(RichText::new(i18n::t(i18n::Key::AboutCopyright, lang)).small());
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add(widgets::rounded_button(
                    i18n::t(i18n::Key::MenuItemRepo, lang),
                    None,
                ))
                .clicked()
            {
                open_repo_url();
            }
            if ui
                .add(widgets::rounded_button(
                    i18n::t(i18n::Key::AboutTitle, lang),
                    None,
                ))
                .clicked()
            {
                *show_about = true;
            }
        });

        // ── 自更新 ──
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);
        let status = updater.snapshot();
        let busy = matches!(
            status.state,
            crate::updater::UpdateState::Checking
                | crate::updater::UpdateState::Downloading
                | crate::updater::UpdateState::Installing
        );
        ui.horizontal(|ui| {
            match status.state {
                crate::updater::UpdateState::Idle
                | crate::updater::UpdateState::UpToDate
                | crate::updater::UpdateState::Error(_) => {
                    if ui
                        .add_enabled(
                            !busy,
                            widgets::rounded_button(i18n::t(i18n::Key::BtnCheckUpdate, lang), None),
                        )
                        .clicked()
                    {
                        updater.check();
                    }
                }
                crate::updater::UpdateState::Available(_) => {
                    if ui
                        .add(widgets::rounded_button(
                            i18n::t(i18n::Key::BtnInstallUpdate, lang),
                            None,
                        ))
                        .clicked()
                    {
                        updater.install();
                    }
                }
                _ => {
                    // Checking / Downloading / Installing：按钮禁用，靠状态文案展示
                    ui.add_enabled(
                        false,
                        widgets::rounded_button(i18n::t(i18n::Key::BtnCheckUpdate, lang), None),
                    );
                }
            }
            ui.small(match &status.state {
                crate::updater::UpdateState::Checking => {
                    egui::RichText::new(i18n::t(i18n::Key::UpdChecking, lang))
                }
                crate::updater::UpdateState::UpToDate => {
                    egui::RichText::new(i18n::t(i18n::Key::UpdLatest, lang))
                }
                crate::updater::UpdateState::Available(v) => {
                    let text = format!("{} v{}", i18n::t(i18n::Key::UpdAvailable, lang), v);
                    egui::RichText::new(text).color(ui.visuals().warn_fg_color)
                }
                crate::updater::UpdateState::Downloading => {
                    let pct = if status.total > 0 {
                        format!(
                            "{} ({:.0}%)",
                            i18n::t(i18n::Key::UpdDownloading, lang),
                            status.done as f64 * 100.0 / status.total as f64
                        )
                    } else {
                        i18n::t(i18n::Key::UpdDownloading, lang).to_string()
                    };
                    egui::RichText::new(pct).color(ui.visuals().text_color())
                }
                crate::updater::UpdateState::Installing => {
                    egui::RichText::new(i18n::t(i18n::Key::UpdInstalling, lang))
                        .color(ui.visuals().warn_fg_color)
                }
                crate::updater::UpdateState::Error(msg) => {
                    if msg == crate::updater::ERR_NETWORK {
                        egui::RichText::new(i18n::t(i18n::Key::UpdNetworkError, lang))
                            .color(ui.visuals().error_fg_color)
                    } else {
                        egui::RichText::new(format!(
                            "{}: {}",
                            i18n::t(i18n::Key::UpdError, lang),
                            msg
                        ))
                        .color(ui.visuals().error_fg_color)
                    }
                }
                crate::updater::UpdateState::Idle => {
                    egui::RichText::new("").color(ui.visuals().text_color())
                }
            });
        });
        // 下载进度条
        if let crate::updater::UpdateState::Downloading = status.state {
            let frac = if status.total > 0 {
                (status.done as f64 / status.total as f64).clamp(0.0, 1.0) as f32
            } else {
                0.0
            };
            ui.add(egui::ProgressBar::new(frac));
        }
    });
}

/// 构建 systemd 服务文件内容
/// 将模板中的 ExecStart 行替换为实际的启动命令，并自动填充当前用户信息
fn build_systemd_service_file(template: &str, cmd: &str) -> String {
    // 获取当前用户名
    let username = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "your-username".to_string());

    // 获取用户 home 目录
    let home_dir = dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("/home/{}", username));

    // 先替换用户信息占位符
    let template = template
        .replace("your-username", &username)
        .replace("/home/your-username", &home_dir);

    let mut lines: Vec<String> = template.lines().map(String::from).collect();
    let mut in_exec_start = false;

    for line in &mut lines {
        if line.starts_with("ExecStart=") {
            *line = format!("ExecStart={}", cmd);
            in_exec_start = true;
        } else if in_exec_start && line.starts_with("    ") {
            // 跳过原模板中 ExecStart 的续行
            line.clear();
        } else {
            in_exec_start = false;
        }
    }

    // 移除连续的空行
    let mut result = Vec::new();
    let mut prev_empty = false;
    for line in lines {
        let is_empty = line.trim().is_empty();
        if !is_empty || !prev_empty {
            result.push(line);
        }
        prev_empty = is_empty;
    }

    result.join("\n")
}
