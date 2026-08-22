use crate::config::settings::{AppSettings, SettingsManager};
use crate::engine::server::ServerManager;
use crate::i18n;
use crate::ui::helper;
use crate::ui::widgets;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::PathBuf;

pub fn ui(
    ui: &mut egui::Ui,
    settings: &mut AppSettings,
    settings_manager: &SettingsManager,
    lang: &i18n::Language,
    #[cfg_attr(not(target_os = "linux"), allow(unused_variables))] server_manager: &ServerManager,
    downloader: &crate::downloader::DownloadHandle,
) {
    // 下载成功时回写 server_path（幂等）
    let snapshot = downloader.snapshot();
    if let crate::downloader::DownloadState::Success(path) = &snapshot.state {
        let new_path = PathBuf::from(path.as_str());
        if settings.server_path != new_path {
            settings.server_path = new_path;
        }
    }

    let accent = crate::theme::accent_color(&settings.accent_color);

    // ── Server 路径 ──
    widgets::card(
        ui,
        i18n::t(i18n::Key::PanelServerTitle, lang),
        accent,
        |ui| {
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
                    settings.llama_version = get_local_llama_version(&settings.server_path);
                }

                if !settings.llama_version.is_empty() {
                    ui.small(egui::RichText::new(&settings.llama_version).weak());
                }
                if settings.update_available == Some(true) {
                    ui.small(
                        egui::RichText::new(i18n::t(i18n::Key::StatusNewVersion, lang)).weak(),
                    );
                }
            });

            ui.separator();

            // llama.cpp 下载子区标题（与参数面板"思考 (Reasoning)"同风格：显式主文本色 + strong）
            ui.label(
                egui::RichText::new(i18n::t(i18n::Key::SubSectionLlamaCppDownload, lang))
                    .color(ui.visuals().text_color())
                    .strong(),
            );

            // 变体选择（按平台展示对应选项）
            let platform_supported = cfg!(target_os = "windows") || cfg!(target_os = "linux");
            let busy = downloader.is_busy();
            let is_linux = cfg!(target_os = "linux");

            // 兼容旧版：把历史 "gpu" 归一化为当前平台默认 GPU 变体（幂等）
            if settings.download_variant == "gpu" {
                settings.download_variant = if is_linux {
                    "vulkan".to_string()
                } else {
                    "cuda124".to_string()
                };
            }

            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut settings.download_variant,
                    "cpu".to_string(),
                    i18n::t(i18n::Key::VariantCpu, lang),
                );
                if !is_linux {
                    ui.selectable_value(
                        &mut settings.download_variant,
                        "cuda124".to_string(),
                        i18n::t(i18n::Key::VariantGpuCuda, lang),
                    );
                    ui.selectable_value(
                        &mut settings.download_variant,
                        "cuda133".to_string(),
                        i18n::t(i18n::Key::VariantGpuCuda133, lang),
                    );
                    ui.selectable_value(
                        &mut settings.download_variant,
                        "rocm714".to_string(),
                        i18n::t(i18n::Key::VariantGpuRocm714, lang),
                    );
                    // 当选择 ROCm 7.14 时显示 GPU 目标选择
                    if settings.download_variant == "rocm714" {
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label(i18n::t(i18n::Key::GpuTargetLabel, lang));
                            egui::ComboBox::from_id_salt("rocm_gpu_target_combo")
                                .selected_text(&settings.rocm_gpu_target)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut settings.rocm_gpu_target,
                                        "gfx103X".to_string(),
                                        i18n::t(i18n::Key::VariantGpuGfx103X, lang),
                                    );
                                    ui.selectable_value(
                                        &mut settings.rocm_gpu_target,
                                        "gfx110X".to_string(),
                                        i18n::t(i18n::Key::VariantGpuGfx110X, lang),
                                    );
                                    ui.selectable_value(
                                        &mut settings.rocm_gpu_target,
                                        "gfx1150".to_string(),
                                        i18n::t(i18n::Key::VariantGpuGfx1150, lang),
                                    );
                                    ui.selectable_value(
                                        &mut settings.rocm_gpu_target,
                                        "gfx1151".to_string(),
                                        i18n::t(i18n::Key::VariantGpuGfx1151, lang),
                                    );
                                    ui.selectable_value(
                                        &mut settings.rocm_gpu_target,
                                        "gfx120X".to_string(),
                                        i18n::t(i18n::Key::VariantGpuGfx120X, lang),
                                    );
                                    ui.selectable_value(
                                        &mut settings.rocm_gpu_target,
                                        "gfx908".to_string(),
                                        i18n::t(i18n::Key::VariantGpuGfx908, lang),
                                    );
                                    ui.selectable_value(
                                        &mut settings.rocm_gpu_target,
                                        "gfx90a".to_string(),
                                        i18n::t(i18n::Key::VariantGpuGfx90a, lang),
                                    );
                                });
                        });
                    }
                }
                ui.selectable_value(
                    &mut settings.download_variant,
                    "vulkan".to_string(),
                    i18n::t(i18n::Key::VariantGpuVulkan, lang),
                );
            });

            // 解析当前选中配置对应的下载变体（平台感知 + 兜底）
            let variant =
                crate::downloader::DownloadVariant::from_settings_value(&settings.download_variant);

            // 下载 llama.cpp + 检查更新：单独一行
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        platform_supported && !busy,
                        egui::Button::new(i18n::t(i18n::Key::BtnDownloadLlamaCpp, lang)),
                    )
                    .clicked()
                {
                    downloader.start_download(settings_manager.config_dir().to_path_buf(), variant);
                }

                // 检查更新：llama-server 路径非空时可用
                if ui
                    .add_enabled(
                        !settings.server_path.to_string_lossy().is_empty(),
                        egui::Button::new(i18n::t(i18n::Key::BtnCheckUpdate, lang)),
                    )
                    .clicked()
                {
                    // 刷新本地版本缓存并获取最新 tag 比对
                    settings.llama_version = get_local_llama_version(&settings.server_path);
                    let variant = crate::downloader::DownloadVariant::from_settings_value(
                        &settings.download_variant,
                    );
                    match crate::downloader::fetch_latest_tag(variant) {
                        Ok(latest) => {
                            // 比对 build 标签（如 b10549）；本地无法解析时视为有新版本
                            let up_to_date =
                                extract_build_tag(&settings.llama_version) == Some(latest);
                            settings.update_available = Some(!up_to_date);
                        }
                        Err(e) => {
                            log::error!("检查更新失败: {}", e);
                        }
                    }
                }
            });

            // 进度/状态行（仅非 Idle 时渲染）
            match &snapshot.state {
                crate::downloader::DownloadState::Running => {
                    let phase_key = match snapshot.phase {
                        crate::downloader::Phase::FetchingRelease => i18n::Key::DlPhaseFetching,
                        crate::downloader::Phase::Downloading => i18n::Key::DlPhaseDownloading,
                        crate::downloader::Phase::Extracting => i18n::Key::DlPhaseExtracting,
                        crate::downloader::Phase::LocatingServer => i18n::Key::DlPhaseLocating,
                    };
                    ui.horizontal(|ui| {
                        ui.label(i18n::t(phase_key, lang));
                        let ratio = snapshot
                            .total
                            .map(|t| {
                                if t > 0 {
                                    snapshot.done as f32 / t as f32
                                } else {
                                    0.0
                                }
                            })
                            .unwrap_or(0.0);
                        ui.add(
                            egui::ProgressBar::new(ratio).text(format!("{:.1}%", ratio * 100.0)),
                        );
                    });
                }
                crate::downloader::DownloadState::Success(_) => {
                    ui.label(i18n::t(i18n::Key::DlSuccess, lang));
                }
                crate::downloader::DownloadState::Error(message) => {
                    ui.label(format!(
                        "{}: {}",
                        i18n::t(i18n::Key::DlFailed, lang),
                        message
                    ));
                }
                crate::downloader::DownloadState::Idle => {}
            }
        },
    );

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

        // 并行槽位
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelParallelSlots, lang));
            ui.add(egui::DragValue::new(&mut settings.parallel_slots).range(1..=1024));
        });

        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelAlias, lang));
            ui.text_edit_singleline(&mut settings.alias);
        });
    });

    // ── 功能开关（Toggle 在左，标签在右）──
    widgets::card(
        ui,
        i18n::t(i18n::Key::SectionFeatures, lang),
        accent,
        |ui| {
            // ★ 新 toggle 签名：toggle(ui, &mut bool, "标签", accent) —— 内部渲染 [开关][标签]
            widgets::toggle(
                ui,
                &mut settings.verbose,
                i18n::t(i18n::Key::CheckboxVerbose, lang),
                accent,
            );
            widgets::toggle(
                ui,
                &mut settings.log_timestamps,
                i18n::t(i18n::Key::CheckboxLogTimestamps, lang),
                accent,
            );
            widgets::toggle(
                ui,
                &mut settings.offline_mode,
                i18n::t(i18n::Key::CheckboxOfflineMode, lang),
                accent,
            );
            widgets::toggle(
                ui,
                &mut settings.rpc_mode,
                i18n::t(i18n::Key::CheckboxRpcMode, lang),
                accent,
            );
            if settings.rpc_mode {
                ui.indent("rpc_endpoints", |ui| {
                    ui.horizontal(|ui| {
                        ui.label(i18n::t(i18n::Key::LabelRpcEndpoints, lang));
                        ui.text_edit_singleline(&mut settings.rpc_endpoints);
                        ui.small(i18n::t(i18n::Key::HintRpcEndpoints, lang));
                    });
                    // 下一行：添加本机 RPC 客户端按钮
                    if ui
                        .button(i18n::t(i18n::Key::BtnAddLocalRpcClient, lang))
                        .clicked()
                    {
                        let new_addr = format!("127.0.0.1:{}", settings.rpc_port);
                        let existing = settings.rpc_endpoints.trim().to_string();
                        settings.rpc_endpoints = if existing.is_empty() {
                            new_addr
                        } else {
                            // 已有配置：新增地址 + 英文逗号 + 原内容
                            format!("{},{}", new_addr, existing)
                        };
                    }
                });
            }
            widgets::toggle(
                ui,
                &mut settings.web_ui_enabled,
                i18n::t(i18n::Key::CheckboxEnableWebClient, lang),
                accent,
            );

            // 日志级别（--log-verbosity）分段控件
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelLogVerbosity, lang));
                let lv_vals = [0usize, 1, 2, 3, 4, 5];
                let lv_labels = [
                    i18n::t(i18n::Key::LogVerbosity0, lang),
                    i18n::t(i18n::Key::LogVerbosity1, lang),
                    i18n::t(i18n::Key::LogVerbosity2, lang),
                    i18n::t(i18n::Key::LogVerbosity3, lang),
                    i18n::t(i18n::Key::LogVerbosity4, lang),
                    i18n::t(i18n::Key::LogVerbosity5, lang),
                ];
                let mut lv_idx = lv_vals
                    .iter()
                    .position(|v| *v == settings.log_verbosity)
                    .unwrap_or(3);
                widgets::segmented(ui, &lv_labels, &mut lv_idx, accent);
                settings.log_verbosity = lv_vals[lv_idx];
            });
        },
    );

    // ── API 安全 / 部署 ──
    widgets::card(
        ui,
        i18n::t(i18n::Key::SectionApiSecurity, lang),
        accent,
        |ui| {
            // API Key
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelApiKey, lang));
                ui.text_edit_singleline(&mut settings.api_key);
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpApiKey, lang));
            });
            // API Prefix
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelApiPrefix, lang));
                ui.text_edit_singleline(&mut settings.api_prefix);
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpApiPrefix, lang));
            });
            // CORS Origins
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelCorsOrigins, lang));
                ui.text_edit_singleline(&mut settings.cors_origins);
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpCorsOrigins, lang));
            });
            // SSL 证书 / 私钥
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelSslCertFile, lang));
                let mut cert_str = settings.ssl_cert_file.to_string_lossy().to_string();
                let resp = ui.text_edit_singleline(&mut cert_str);
                if resp.changed() {
                    settings.ssl_cert_file = PathBuf::from(&cert_str);
                }
                if ui.button(i18n::t(i18n::Key::BtnBrowseCert, lang)).clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title(i18n::t(i18n::Key::LabelSslCertFile, lang))
                        .add_filter("PEM", &["pem", "crt", "cer"])
                        .pick_file()
                    {
                        settings.ssl_cert_file = path;
                    }
                }
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpSsl, lang));
            });
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelSslKeyFile, lang));
                let mut key_str = settings.ssl_key_file.to_string_lossy().to_string();
                let resp = ui.text_edit_singleline(&mut key_str);
                if resp.changed() {
                    settings.ssl_key_file = PathBuf::from(&key_str);
                }
                if ui.button(i18n::t(i18n::Key::BtnBrowseCert, lang)).clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title(i18n::t(i18n::Key::LabelSslKeyFile, lang))
                        .add_filter("PEM", &["pem", "key"])
                        .pick_file()
                    {
                        settings.ssl_key_file = path;
                    }
                }
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpSsl, lang));
            });
            // 端口复用（开关在最左面）
            ui.horizontal(|ui| {
                widgets::toggle(
                    ui,
                    &mut settings.reuse_port,
                    i18n::t(i18n::Key::CheckboxReusePort, lang),
                    accent,
                );
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpReusePort, lang));
            });
            // NUMA 模式
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelNuma, lang));
                let numa_vals = ["", "distribute", "isolate", "numactl"];
                let numa_labels = [
                    i18n::t(i18n::Key::LoadModeAuto, lang),
                    numa_vals[1],
                    numa_vals[2],
                    numa_vals[3],
                ];
                let mut numa_idx = numa_vals
                    .iter()
                    .position(|v| *v == settings.numa)
                    .unwrap_or(0);
                widgets::segmented(ui, &numa_labels, &mut numa_idx, accent);
                settings.numa = numa_vals[numa_idx].to_string();
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpNuma, lang));
            });
        },
    );
}

/// 运行 llama-server --version，返回解析出的版本字符串
fn get_local_llama_version(server_path: &std::path::Path) -> String {
    let mut cmd = std::process::Command::new(server_path);
    cmd.arg("--version")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x0800_0000u32); // CREATE_NO_WINDOW
    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let all_lines: Vec<&str> = stdout.lines().chain(stderr.lines()).collect();
            // 兼容 "version: xxx" 与 llama.cpp 的 "version = xxx (bxxx, hash)" 两种格式
            let version_line = all_lines
                .iter()
                .find(|line| line.contains("version:"))
                .or_else(|| {
                    all_lines
                        .iter()
                        .find(|line| line.trim_start().starts_with("version"))
                });
            version_line
                .and_then(|line| {
                    // 去掉 "version:" / "version =" 前缀，保留版本号部分
                    let value = if let Some(v) = line.split_once("version:") {
                        v.1
                    } else if let Some(v) = line.split_once("version") {
                        v.1
                    } else {
                        return None;
                    };
                    let trimmed = value.trim().trim_start_matches('=').trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                })
                .unwrap_or_else(|| "未知版本".to_string())
        }
        Err(e) => format!("获取失败: {}", e),
    }
}

/// 从版本字符串中提取 build 标签（如 "b10549"）
fn extract_build_tag(version: &str) -> Option<String> {
    let bytes = version.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'b' && bytes[i + 1].is_ascii_digit() {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            return Some(format!("b{}", &version[start..end]));
        }
        i += 1;
    }
    None
}
