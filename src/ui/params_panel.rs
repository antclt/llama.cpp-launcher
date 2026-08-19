use crate::config::settings::{is_server_binary_name, AppSettings, GpuLayersMode};
use crate::i18n;
use crate::kv_cache;
use crate::ui::helper;
use crate::ui::widgets;
use std::path::PathBuf;

pub fn ui(ui: &mut egui::Ui, settings: &mut AppSettings, lang: &i18n::Language) {
    let accent = crate::theme::accent_color(&settings.accent_color);

    let server_path_valid = settings
        .server_path
        .file_name()
        .and_then(|f| f.to_str())
        .is_some_and(is_server_binary_name);
    let can_start = server_path_valid && !settings.model_path.as_os_str().is_empty();

    // ── 上下文与批次 ──
    widgets::card(ui, i18n::t(i18n::Key::SectionContextBatch, lang), accent, |ui| {
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
            .clicked() && can_start
        {
            match kv_cache::calc_max_context_facade(settings) {
                Ok(val) => settings.context = val,
                Err(e) => log::warn!("[params_panel] calc_max_context 失败: {}", e),
            }
        }

        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelBatchSize, lang));
            ui.add(
                egui::DragValue::new(&mut settings.batch_size)
                    .range(1..=16)
                    .speed(1),
            );
            ui.label("k");
            ui.small(i18n::t(i18n::Key::HintKUnit, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpBatchSize, lang));
        });

        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelUBatchSize, lang));
            ui.add(
                egui::DragValue::new(&mut settings.ubatch_size)
                    .range(0.5..=16.0)
                    .speed(0.5),
            );
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
            );
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
                .clicked() && can_start
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
    });

    // ── 采样参数 ──
    widgets::card(ui, i18n::t(i18n::Key::SectionSampling, lang), accent, |ui| {
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelTemperature, lang));
            ui.add(
                egui::Slider::new(&mut settings.temperature, 0.0..=2.0)
                    .smallest_positive(0.01)
                    .custom_formatter(|v, _| format!("{:.2}", v)),
            );
            ui.label(format!("{:.2}", settings.temperature));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpTemperature, lang));
            // ★ Toggle 新签名：开关在左，标签在右
        widgets::toggle(ui, &mut settings.ignore_temperature, "", accent);
        });

        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelTopP, lang));
            ui.add(
                egui::Slider::new(&mut settings.top_p, 0.0..=1.0)
                    .smallest_positive(0.01)
                    .custom_formatter(|v, _| format!("{:.2}", v)),
            );
            ui.label(format!("{:.2}", settings.top_p));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpTopP, lang));
            widgets::toggle(ui, &mut settings.ignore_top_p, "", accent);
        });

        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelTopK, lang));
            ui.add(egui::DragValue::new(&mut settings.top_k).range(0..=1000));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpTopK, lang));
            widgets::toggle(ui, &mut settings.ignore_top_k, "", accent);
        });

        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelRepeatPenalty, lang));
            ui.add(
                egui::Slider::new(&mut settings.repeat_penalty, 0.0..=2.0)
                    .smallest_positive(0.01)
                    .custom_formatter(|v, _| format!("{:.2}", v)),
            );
            ui.label(format!("{:.2}", settings.repeat_penalty));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpRepeatPenalty, lang));
            widgets::toggle(ui, &mut settings.ignore_repeat_penalty, "", accent);
        });

        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelPresencePenalty, lang));
            ui.add(
                egui::Slider::new(&mut settings.presence_penalty, -2.0..=2.0)
                    .smallest_positive(0.01)
                    .custom_formatter(|v, _| format!("{:.2}", v)),
            );
            ui.label(format!("{:.2}", settings.presence_penalty));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpPresencePenalty, lang));
            widgets::toggle(ui, &mut settings.ignore_presence_penalty, "", accent);
        });
    });

    // ── 思考控制（Reasoning）──
    widgets::card(ui, i18n::t(i18n::Key::SectionReasoning, lang), accent, |ui| {
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelReasoningMode, lang));
            let rm_vals = ["auto", "on", "off"];
            let rm_labels = [
                i18n::t(i18n::Key::ReasoningModeAuto, lang),
                i18n::t(i18n::Key::ReasoningModeOn, lang),
                i18n::t(i18n::Key::ReasoningModeOff, lang),
            ];
            let mut rm_idx = rm_vals
                .iter()
                .position(|v| *v == settings.reasoning_mode)
                .unwrap_or(0);
            widgets::segmented(ui, &rm_labels, &mut rm_idx, accent);
            settings.reasoning_mode = rm_vals[rm_idx].to_string();
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpReasoningMode, lang));
        });

        ui.label(i18n::t(i18n::Key::LabelReasoningEffort, lang));
        let efforts = ["default", "minimal", "low", "medium", "high", "xhigh", "max"];
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            for e in &efforts {
                let selected = settings.reasoning_effort == *e;
                if ui.selectable_label(selected, *e).clicked() {
                    settings.reasoning_effort = e.to_string();
                }
            }
        });
        ui.horizontal(|ui| {
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpReasoningEffort, lang));
        });

        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelReasoningFormat, lang));
            let rf_vals = ["auto", "none", "deepseek", "deepseek-legacy"];
            let mut rf_idx = rf_vals
                .iter()
                .position(|v| *v == settings.reasoning_format)
                .unwrap_or(0);
            widgets::segmented(ui, &rf_vals, &mut rf_idx, accent);
            settings.reasoning_format = rf_vals[rf_idx].to_string();
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpReasoningFormat, lang));
        });

        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelReasoningPreserve, lang));
            let rp_vals = ["", "on", "off"];
            let rp_labels = [
                i18n::t(i18n::Key::ReasoningPreserveDefault, lang),
                i18n::t(i18n::Key::ReasoningPreserveOn, lang),
                i18n::t(i18n::Key::ReasoningPreserveOff, lang),
            ];
            let mut rp_idx = match settings.reasoning_preserve.as_str() {
                "on" => 1,
                "off" => 2,
                _ => 0,
            };
            widgets::segmented(ui, &rp_labels, &mut rp_idx, accent);
            settings.reasoning_preserve = rp_vals[rp_idx].to_string();
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpReasoningPreserve, lang));
        });

        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelReasoningBudget, lang));
            ui.add(
                egui::DragValue::new(&mut settings.reasoning_budget)
                    .range(-1..=65536)
                    .speed(128),
            );
            ui.small(i18n::t(i18n::Key::HintReasoningBudget, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpReasoningBudget, lang));
        });

        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelReasoningBudgetMessage, lang));
            ui.text_edit_singleline(&mut settings.reasoning_budget_message);
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpReasoningBudgetMessage, lang));
        });
    });

    // ── 聊天模板 ──
    widgets::card(ui, i18n::t(i18n::Key::SectionChatTemplate, lang), accent, |ui| {
        widgets::toggle(
            ui,
            &mut settings.jinja_enabled,
            i18n::t(i18n::Key::CheckboxJinja, lang),
            accent,
        );
        helper::help_button_inline(ui, i18n::t(i18n::Key::HelpJinja, lang));

        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelChatTemplateFile, lang));
            let mut path_str = settings.chat_template_file.to_string_lossy().to_string();
            let resp = ui.text_edit_singleline(&mut path_str);
            if resp.changed() {
                settings.chat_template_file = PathBuf::from(&path_str);
            }
            if ui.button(i18n::t(i18n::Key::BtnBrowse, lang)).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title(i18n::t(i18n::Key::DialogSelectTemplate, lang))
                    .add_filter(
                        i18n::t(i18n::Key::FilterTemplate, lang),
                        &["jinja", "jinja2", "txt", "json"],
                    )
                    .pick_file()
                {
                    settings.chat_template_file = path;
                }
            }
            if ui.button(i18n::t(i18n::Key::BtnClear, lang)).clicked() {
                settings.chat_template_file = PathBuf::new();
            }
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpChatTemplateFile, lang));
        });
    });

    // ── Flash Attention ──
    widgets::card(ui, i18n::t(i18n::Key::LabelFlashAttn, lang), accent, |ui| {
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

    // ── KV 缓存配置 ──
    widgets::card(ui, i18n::t(i18n::Key::SectionKvCache, lang), accent, |ui| {
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelLoadMode, lang));
            let lm_vals = ["auto", "none", "mmap", "mlock", "mmap+mlock", "dio"];
            let lm_labels = [
                i18n::t(i18n::Key::LoadModeAuto, lang),
                lm_vals[1],
                lm_vals[2],
                lm_vals[3],
                lm_vals[4],
                lm_vals[5],
            ];
            let mut lm_idx = lm_vals
                .iter()
                .position(|v| *v == settings.load_mode)
                .unwrap_or(0);
            widgets::segmented(ui, &lm_labels, &mut lm_idx, accent);
            settings.load_mode = lm_vals[lm_idx].to_string();
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpLoadMode, lang));
        });

        ui.horizontal(|ui| {
            // ★ Toggle 新签名：开关在左，标签在右
            widgets::toggle(ui, &mut settings.kv_offload, i18n::t(i18n::Key::CheckboxKvOffload, lang), accent);
        });
        ui.small(i18n::t(i18n::Key::HintKvOffload, lang));

        ui.label(i18n::t(i18n::Key::LabelCacheTypeK, lang));
        let k_types = ["f32", "f16", "bf16", "q8_0", "q4_0", "q4_1", "iq4_nl", "q5_0", "q5_1"];
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            for k_type in &k_types {
                let selected = settings.cache_type_k == *k_type;
                if ui.selectable_label(selected, *k_type).clicked() {
                    settings.cache_type_k = k_type.to_string();
                }
            }
        });
        ui.horizontal(|ui| {
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpCacheTypeK, lang));
        });

        ui.label(i18n::t(i18n::Key::LabelCacheTypeV, lang));
        let v_types = ["f32", "f16", "bf16", "q8_0", "q4_0", "q4_1", "iq4_nl", "q5_0", "q5_1"];
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            for v_type in &v_types {
                let selected = settings.cache_type_v == *v_type;
                if ui.selectable_label(selected, *v_type).clicked() {
                    settings.cache_type_v = v_type.to_string();
                }
            }
        });
        ui.horizontal(|ui| {
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpCacheTypeV, lang));
        });

        for (label_key, _help_key, flag) in [
            (i18n::Key::CheckboxKvMlock, i18n::Key::HelpKvMlock, &mut settings.kv_mlock),
            (i18n::Key::CheckboxKvMmap, i18n::Key::HelpKvMmap, &mut settings.kv_mmap),
            (i18n::Key::CheckboxKvUnified, i18n::Key::HelpKvUnified, &mut settings.kv_unified),
            (i18n::Key::CheckboxSwaFull, i18n::Key::HelpSwaFull, &mut settings.swa_full),
        ] {
            // ★ Toggle 新签名：开关在左，标签在右
            widgets::toggle(ui, flag, i18n::t(label_key, lang), accent);
        }

        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelCtxCheckpoints, lang));
            ui.add(
                egui::DragValue::new(&mut settings.ctx_checkpoints)
                    .range(1..=256)
                    .speed(1),
            );
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpCtxCheckpoints, lang));
        });

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
    widgets::card(ui, i18n::t(i18n::Key::SectionGpuDevice, lang), accent, |ui| {
        let mut manual_gpu_layers = matches!(settings.gpu_layers_mode, GpuLayersMode::Manual(_));
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

        ui.horizontal(|ui| {
            // 修复：原先 ui.label 与 toggle 各渲染一次标签，导致“手动指定 GPU 层数”重复显示
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpGpuDevice, lang));
            // ★ Toggle 新签名
            widgets::toggle(ui, &mut manual_gpu_layers, i18n::t(i18n::Key::CheckboxManualGpuLayers, lang), accent);
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

        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelTensorSplit, lang));
            ui.text_edit_singleline(&mut settings.tensor_split);
            ui.small(i18n::t(i18n::Key::HintTensorSplit, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpTensorSplit, lang));
        });

        ui.horizontal(|ui| {
            // 修复：原先 ui.label 与 toggle 各渲染一次标签，导致“CPU MoE”重复显示
            ui.small(i18n::t(i18n::Key::HintCpuMoe, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpCpuMoe, lang));
            // ★ Toggle 新签名
            widgets::toggle(ui, &mut settings.cpu_moe, i18n::t(i18n::Key::CheckboxCpuMoe, lang), accent);
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
    });

    // ── 推测解码 ──
    widgets::card(ui, i18n::t(i18n::Key::SectionSpecDecoding, lang), accent, |ui| {
        ui.label(i18n::t(i18n::Key::SpecTypeLabel, lang));
        let spec_options = [
            "none", "draft-simple", "draft-eagle3", "draft-mtp", "ngram-simple",
            "ngram-map-k", "ngram-map-k4v", "ngram-mod", "ngram-cache", "dflash",
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
        ui.horizontal(|ui| {
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpSpecType, lang));
        });

        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::SpecDraftNMaxLabel, lang));
            ui.add(egui::DragValue::new(&mut settings.spec_draft_n_max).range(0..=64));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpSpecDraftNMax, lang));
        });

        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::SpecDraftNMinLabel, lang));
            ui.add(egui::DragValue::new(&mut settings.spec_draft_n_min).range(0..=32));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpSpecDraftNMin, lang));
        });

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
    });
}
