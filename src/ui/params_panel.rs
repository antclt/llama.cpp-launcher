use crate::config::settings::{is_server_binary_name, AppSettings, GpuLayersMode};
use crate::i18n;
use crate::kv_cache;
use crate::ui::helper;
use crate::ui::widgets;

pub fn ui(ui: &mut egui::Ui, settings: &mut AppSettings, lang: &i18n::Language) {
    let accent = crate::theme::accent_color(&settings.accent_color);

    let server_path_valid = settings
        .server_path
        .file_name()
        .and_then(|f| f.to_str())
        .is_some_and(is_server_binary_name);
    let can_start = server_path_valid && !settings.model_path.as_os_str().is_empty();

    // ── 上下文与批次 ──
    widgets::card(
        ui,
        i18n::t(i18n::Key::SectionContextBatch, lang),
        accent,
        |ui| {
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelNCtx, lang));
                ui.add(
                    egui::DragValue::new(&mut settings.context)
                        .range(1..=1024)
                        .speed(1),
                );
                ui.label("k");
                ui.small(i18n::t(i18n::Key::HintKUnit, lang));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpNCtx, lang));
            });
            if ui
                .button(i18n::t(i18n::Key::BtnSetMaxContextVram, lang))
                .clicked()
                && can_start
            {
                match kv_cache::calc_max_context_facade(settings) {
                    Ok(val) => settings.context = val,
                    Err(e) => log::warn!("[params_panel] calc_max_context 失败: {}", e),
                }
            }
            // 批次大小 (--batch-size) (k)
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelBatchSize, lang));
                ui.add(
                    egui::DragValue::new(&mut settings.batch_size)
                        .range(1..=16)
                        .speed(1),
                ); // 1k ~ 16k
                ui.label("k");
                ui.small(i18n::t(i18n::Key::HintKUnit, lang));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpBatchSize, lang));
            });
            // 物理批次大小 (--ubatch-size) (k)
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelUBatchSize, lang));
                ui.add(
                    egui::DragValue::new(&mut settings.ubatch_size)
                        .range(0.5..=16.0)
                        .speed(0.5),
                ); // 0.5k ~ 16k, 步进 0.5
                ui.label("k");
                ui.small(i18n::t(i18n::Key::HintKUnit, lang));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpUBatchSize, lang));
            });

            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelSessionTimeout, lang));
                ui.add(
                    egui::DragValue::new(&mut settings.session_timeout)
                        .range(60..=3600)
                        .speed(10),
                ); // 60~3600秒，步进10
                ui.label(i18n::t(i18n::Key::HintSUnit, lang));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpSessionTimeout, lang));
            });

            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelKvCacheRatio, lang));
                ui.add(
                    egui::DragValue::new(&mut settings.kv_cache_ratio)
                        .range(0.0..=1.0)
                        .speed(0.01),
                );
                ui.label(format!("{:.2}", settings.kv_cache_ratio));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpKvCacheRatio, lang));
            });

            ui.horizontal(|ui| {
                if ui
                    .button(i18n::t(i18n::Key::BtnCalcKvCache, lang))
                    .clicked()
                    && can_start
                {
                    settings.kv_cache_result = match kv_cache::calc_and_format(settings) {
                        Ok(result) => Some(format!(
                            "{} {}",
                            i18n::t(i18n::Key::LabelKvCacheResult, lang),
                            result
                        )),
                        Err(e) => Some(format!("⚠ {}", e)),
                    };
                }
                if let Some(ref result) = settings.kv_cache_result {
                    ui.small(egui::RichText::new(result).weak());
                }
            });

            // ── Flash Attention（上下文与批次 子项）──
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelFlashAttn, lang));
                let fa_vals = ["on", "off", "auto"];
                let fa_labels = [
                    i18n::t(i18n::Key::FaModeOn, lang),
                    i18n::t(i18n::Key::FaModeOff, lang),
                    i18n::t(i18n::Key::FaModeAuto, lang),
                ];
                let mut fa_idx = fa_vals
                    .iter()
                    .position(|v| *v == settings.flash_attn)
                    .unwrap_or(2);
                widgets::segmented(ui, &fa_labels, &mut fa_idx, accent);
                settings.flash_attn = fa_vals[fa_idx].to_string();
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpFlashAttn, lang));
            });
        },
    );

    // ── 思考与会话 ──
    widgets::card(
        ui,
        i18n::t(i18n::Key::SectionThinkingConversation, lang),
        accent,
        |ui| {
            // 思考子区标题
            // ★ 不用 .strong()（浅色模式下 strong_text_color=白色→隐形），改用显式主文本色
            ui.label(
                egui::RichText::new(i18n::t(i18n::Key::SubSectionThinking, lang))
                    .color(ui.visuals().text_color())
                    .strong(),
            );

            // 推理模式 (--reasoning)
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelReasoning, lang));
                let r_vals = ["auto", "on", "off"];
                let r_labels = [
                    i18n::t(i18n::Key::ReasoningModeAuto, lang),
                    i18n::t(i18n::Key::ReasoningModeOn, lang),
                    i18n::t(i18n::Key::ReasoningModeOff, lang),
                ];
                let mut r_idx = r_vals
                    .iter()
                    .position(|v| *v == settings.reasoning)
                    .unwrap_or(0);
                widgets::segmented(ui, &r_labels, &mut r_idx, accent);
                settings.reasoning = r_vals[r_idx].to_string();
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpReasoning, lang));
            });

            // 思考格式 (--reasoning-format)
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelReasoningFormat, lang));
                let rf_vals = ["auto", "none", "deepseek", "deepseek-legacy"];
                let rf_labels = [
                    i18n::t(i18n::Key::ReasoningFormatAuto, lang),
                    i18n::t(i18n::Key::ReasoningFormatNone, lang),
                    i18n::t(i18n::Key::ReasoningFormatDeepseek, lang),
                    i18n::t(i18n::Key::ReasoningFormatDeepseekLegacy, lang),
                ];
                let mut rf_idx = rf_vals
                    .iter()
                    .position(|v| *v == settings.reasoning_format)
                    .unwrap_or(0);
                widgets::segmented(ui, &rf_labels, &mut rf_idx, accent);
                settings.reasoning_format = rf_vals[rf_idx].to_string();
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpReasoningFormat, lang));
            });

            // 推理强度 (--reasoning-effort)：标签 + ❓提示框同一行
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelReasoningEffort, lang));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpReasoningEffort, lang));
            });
            let effort_vals = [
                "default", "minimal", "low", "medium", "high", "xhigh", "max",
            ];
            let effort_labels = [
                i18n::t(i18n::Key::EffortDefault, lang),
                i18n::t(i18n::Key::EffortMinimal, lang),
                i18n::t(i18n::Key::EffortLow, lang),
                i18n::t(i18n::Key::EffortMedium, lang),
                i18n::t(i18n::Key::EffortHigh, lang),
                i18n::t(i18n::Key::EffortXhigh, lang),
                i18n::t(i18n::Key::EffortMax, lang),
            ];
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;

                for (i, opt) in effort_vals.iter().enumerate() {
                    let selected = settings.reasoning_effort == *opt;
                    if ui.selectable_label(selected, effort_labels[i]).clicked() {
                        settings.reasoning_effort = opt.to_string();
                    }
                }
            });

            // 思考预算 (--reasoning-budget)
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelReasoningBudget, lang));
                ui.add(
                    egui::DragValue::new(&mut settings.reasoning_budget)
                        .range(-1..=32768)
                        .speed(1),
                );
                ui.small(i18n::t(i18n::Key::HintReasoningBudget, lang));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpReasoningBudget, lang));
            });

            ui.separator();

            // 会话子区标题
            // ★ 不用 .strong()（浅色模式下 strong_text_color=白色→隐形），改用显式主文本色
            ui.label(
                egui::RichText::new(i18n::t(i18n::Key::SubSectionChat, lang))
                    .color(ui.visuals().text_color())
                    .strong(),
            );

            // Jinja 对话模板引擎开关：标签 + ❓提示框 + 开关
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::CheckboxJinja, lang));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpJinja, lang));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    widgets::toggle(ui, &mut settings.jinja_enabled, "", accent);
                });
            });

            // 对话模板 (--chat-template)
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelChatTemplate, lang));
                ui.text_edit_singleline(&mut settings.chat_template);
                ui.small(i18n::t(i18n::Key::HintChatTemplate, lang));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpChatTemplate, lang));
            });

            // 对话模板文件 (--chat-template-file)
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelChatTemplateFile, lang));
                let mut file_str = settings.chat_template_file.to_string_lossy().to_string();
                let response = ui.text_edit_singleline(&mut file_str);
                if response.changed() {
                    settings.chat_template_file = std::path::PathBuf::from(&file_str);
                }
                if ui.button(i18n::t(i18n::Key::BtnBrowse, lang)).clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title(i18n::t(i18n::Key::DialogSelectChatTemplate, lang))
                        .add_filter(
                            i18n::t(i18n::Key::FilterTextFiles, lang),
                            &["txt", "jinja", "j2"],
                        )
                        .pick_file()
                    {
                        settings.chat_template_file = path;
                    }
                }
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpChatTemplateFile, lang));
            });
        },
    );

    // ── 采样参数 ──
    widgets::card(
        ui,
        i18n::t(i18n::Key::SectionSampling, lang),
        accent,
        |ui| {
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelTemperature, lang));
                ui.add(
                    egui::Slider::new(&mut settings.temperature, 0.0..=2.0)
                        .smallest_positive(0.01)
                        .custom_formatter(|v, _| format!("{:.2}", v)),
                );
                ui.label(format!("{:.2}", settings.temperature));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpTemperature, lang));
                // 开关推到行最右侧（与顶栏 right_to_left 右对齐模式一致）
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    widgets::toggle(ui, &mut settings.enable_temperature, "", accent);
                });
            });
            // top_p
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelTopP, lang));
                ui.add(
                    egui::Slider::new(&mut settings.top_p, 0.0..=1.0)
                        .smallest_positive(0.01)
                        .custom_formatter(|v, _| format!("{:.2}", v)),
                );
                ui.label(format!("{:.2}", settings.top_p));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpTopP, lang));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    widgets::toggle(ui, &mut settings.enable_top_p, "", accent);
                });
            });
            // top_k
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelTopK, lang));
                ui.add(egui::DragValue::new(&mut settings.top_k).range(0..=1000));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpTopK, lang));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    widgets::toggle(ui, &mut settings.enable_top_k, "", accent);
                });
            });
            // 重复惩罚
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelRepeatPenalty, lang));
                ui.add(
                    egui::Slider::new(&mut settings.repeat_penalty, 0.0..=2.0)
                        .smallest_positive(0.01)
                        .custom_formatter(|v, _| format!("{:.2}", v)),
                );
                ui.label(format!("{:.2}", settings.repeat_penalty));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpRepeatPenalty, lang));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    widgets::toggle(ui, &mut settings.enable_repeat_penalty, "", accent);
                });
            });
            // 存在惩罚
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelPresencePenalty, lang));
                ui.add(
                    egui::Slider::new(&mut settings.presence_penalty, -2.0..=2.0)
                        .smallest_positive(0.01)
                        .custom_formatter(|v, _| format!("{:.2}", v)),
                );
                ui.label(format!("{:.2}", settings.presence_penalty));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpPresencePenalty, lang));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    widgets::toggle(ui, &mut settings.enable_presence_penalty, "", accent);
                });
            });
        },
    );

    // ── KV 缓存配置 ──
    widgets::card(ui, i18n::t(i18n::Key::SectionKvCache, lang), accent, |ui| {
        // KV 缓存开关统一样式（与「手动指定 GPU 层数」一致）：标签 + ❓提示框 + 开关
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::CheckboxKvOffload, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpKvOffload, lang));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                widgets::toggle(ui, &mut settings.kv_offload, "", accent);
            });
        });

        // K 缓存类型：标签 + ❓提示框同一行
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelCacheTypeK, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpCacheTypeK, lang));
        });
        let k_types = [
            "f32", "f16", "bf16", "q8_0", "q4_0", "q4_1", "iq4_nl", "q5_0", "q5_1",
        ];

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;

            for k_type in &k_types {
                let selected = settings.cache_type_k == *k_type;
                if ui.selectable_label(selected, *k_type).clicked() {
                    settings.cache_type_k = k_type.to_string();
                }
            }
        });

        // V 缓存类型：标签 + ❓提示框同一行
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelCacheTypeV, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpCacheTypeV, lang));
        });
        let v_types = [
            "f32", "f16", "bf16", "q8_0", "q4_0", "q4_1", "iq4_nl", "q5_0", "q5_1",
        ];
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;

            for v_type in &v_types {
                let selected = settings.cache_type_v == *v_type;
                if ui.selectable_label(selected, *v_type).clicked() {
                    settings.cache_type_v = v_type.to_string();
                }
            }
        });

        // KV 缓存开关统一样式（与「手动指定 GPU 层数」一致）：标签 + ❓提示框 + 开关
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::CheckboxKvMlock, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpKvMlock, lang));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                widgets::toggle(ui, &mut settings.kv_mlock, "", accent);
            });
        });
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::CheckboxKvMmap, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpKvMmap, lang));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                widgets::toggle(ui, &mut settings.kv_mmap, "", accent);
            });
        });
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::CheckboxKvUnified, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpKvUnified, lang));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                widgets::toggle(ui, &mut settings.kv_unified, "", accent);
            });
        });

        // 完整滑动窗口 (--swa-full)，与「手动指定 GPU 层数」一致：标签 + ❓提示框 + 开关
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::CheckboxSwaFull, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpSwaFull, lang));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                widgets::toggle(ui, &mut settings.swa_full, "", accent);
            });
        });
        // 上下文检查点
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelCtxCheckpoints, lang));
            ui.add(
                egui::DragValue::new(&mut settings.ctx_checkpoints)
                    .range(1..=256)
                    .speed(1),
            );
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpCtxCheckpoints, lang));
        });
        // 最小检查点步长
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelCheckpointMinStep, lang));
            ui.add(
                egui::DragValue::new(&mut settings.checkpoint_min_step)
                    .range(64..=4096)
                    .speed(64),
            );
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpCheckpointMinStep, lang));
        });
    });

    // ── GPU 与设备分配 ──
    widgets::card(
        ui,
        i18n::t(i18n::Key::SectionGpuDevice, lang),
        accent,
        |ui| {
            let mut manual_gpu_layers =
                matches!(settings.gpu_layers_mode, GpuLayersMode::Manual(_));
            let mut gpu_layers = match settings.gpu_layers_mode {
                GpuLayersMode::Auto => 0usize,
                GpuLayersMode::All => 256usize,
                GpuLayersMode::Manual(n) => n,
            };

            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelGpuDevice, lang));
                let gm_labels = [
                    i18n::t(i18n::Key::GpuModeAuto, lang),
                    i18n::t(i18n::Key::GpuModeAll, lang),
                ];
                let mut gm_idx = match settings.gpu_layers_mode {
                    GpuLayersMode::Auto => 0,
                    GpuLayersMode::All => 1,
                    GpuLayersMode::Manual(_) => 0,
                };
                widgets::segmented(ui, &gm_labels, &mut gm_idx, accent);
                match gm_idx {
                    0 => settings.gpu_layers_mode = GpuLayersMode::Auto,
                    1 => settings.gpu_layers_mode = GpuLayersMode::All,
                    _ => {}
                }
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpGpuDevice, lang));
            });
            // 手动指定 GPU 层数
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::CheckboxManualGpuLayers, lang));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpGpuDevice, lang));
                // ★ Toggle 新签名（行首已有标签，开关后不再重复文字）
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    widgets::toggle(ui, &mut manual_gpu_layers, "", accent);
                });
            });
            if manual_gpu_layers {
                ui.indent("manual_gpu_layers_options", |ui| {
                    ui.horizontal(|ui| {
                        ui.label(i18n::t(i18n::Key::LabelGpuDevice, lang));
                        ui.add(egui::DragValue::new(&mut gpu_layers).range(0..=256));
                        ui.small(i18n::t(i18n::Key::HintGpuDevice, lang));
                    });
                });
                settings.gpu_layers_mode = GpuLayersMode::Manual(gpu_layers);
            }
            // 拆分模式
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelSplitMode, lang));
                let sm_vals = ["none", "layer", "tensor"];
                let sm_labels = [
                    i18n::t(i18n::Key::SplitModeNone, lang),
                    i18n::t(i18n::Key::SplitModeLayer, lang),
                    i18n::t(i18n::Key::SplitModeTensor, lang),
                ];
                let mut sm_idx = sm_vals
                    .iter()
                    .position(|v| *v == settings.split_mode)
                    .unwrap_or(0);
                widgets::segmented(ui, &sm_labels, &mut sm_idx, accent);
                settings.split_mode = sm_vals[sm_idx].to_string();
                ui.small(i18n::t(i18n::Key::HintSplitMode, lang));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpSplitMode, lang));
            });
            // 张量拆分比例
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelTensorSplit, lang));
                ui.text_edit_singleline(&mut settings.tensor_split);
                ui.small(i18n::t(i18n::Key::HintTensorSplit, lang));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpTensorSplit, lang));
            });
            // CPU MoE（与 RPC 模式一致的缩进样式）
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::CheckboxCpuMoe, lang));
                ui.small(i18n::t(i18n::Key::HintCpuMoe, lang));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpCpuMoe, lang));
                // ★ Toggle 新签名（行首已有标签，开关后不再重复文字）
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    widgets::toggle(ui, &mut settings.cpu_moe, "", accent);
                });
            });
            if settings.cpu_moe {
                ui.indent("cpu_moe_options", |ui| {
                    ui.horizontal(|ui| {
                        ui.label(i18n::t(i18n::Key::LabelNCpuMoe, lang));
                        ui.add(egui::DragValue::new(&mut settings.n_cpu_moe).range(0..=256));
                        ui.small(i18n::t(i18n::Key::HintNCpuMoe, lang));
                    });
                });
            }
            // 指定特定张量到缓冲区
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelOverrideTensor, lang));
                ui.text_edit_singleline(&mut settings.override_tensor);
                ui.small(i18n::t(i18n::Key::HintOverrideTensor, lang));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpOverrideTensor, lang));
            });
        },
    );

    // ── 推测解码 ──
    widgets::card(
        ui,
        i18n::t(i18n::Key::SectionSpecDecoding, lang),
        accent,
        |ui| {
            // 算法类型：标签 + ❓提示框同一行
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::SpecTypeLabel, lang));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpSpecType, lang));
            });

            let spec_options = [
                "none",
                "draft-simple",
                "draft-eagle3",
                "draft-mtp",
                "ngram-simple",
                "ngram-map-k",
                "ngram-map-k4v",
                "ngram-mod",
                "ngram-cache",
                "dflash",
            ];

            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;

                for opt in &spec_options[..] {
                    let selected = settings.spec_type == *opt;
                    if ui.selectable_label(selected, *opt).clicked() {
                        settings.spec_type = opt.to_string();
                    }
                }
            });
            // 最大推测数量 --spec-draft-n-max（DragValue）
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::SpecDraftNMaxLabel, lang));
                ui.add(egui::DragValue::new(&mut settings.spec_draft_n_max).range(0..=64));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpSpecDraftNMax, lang));
            });
            // 最小推测数量 --spec-draft-n-min（DragValue）
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::SpecDraftNMinLabel, lang));
                ui.add(egui::DragValue::new(&mut settings.spec_draft_n_min).range(0..=32));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpSpecDraftNMin, lang));
            });
            // 信任度 --spec-draft-p-min（Slider）
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::SpecDraftPMinLabel, lang));
                ui.add(
                    egui::Slider::new(&mut settings.spec_draft_p_min, 0.0..=1.0)
                        .smallest_positive(0.01)
                        .custom_formatter(|v, _| format!("{:.2}", v)),
                );
                ui.label(format!("{:.2}", settings.spec_draft_p_min));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpSpecDraftPMin, lang));
            });
            // 分裂概率 --spec-draft-p-split（Slider）
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::SpecDraftPSplitLabel, lang));
                ui.add(
                    egui::Slider::new(&mut settings.spec_draft_p_split, 0.0..=1.0)
                        .smallest_positive(0.01)
                        .custom_formatter(|v, _| format!("{:.2}", v)),
                );
                ui.label(format!("{:.2}", settings.spec_draft_p_split));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpSpecDraftPSplit, lang));
            });
        },
    );
}
