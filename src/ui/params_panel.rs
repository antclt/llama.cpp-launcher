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

    // ── 线程与生成长度 ──
    widgets::card(ui, i18n::t(i18n::Key::SectionThreads, lang), accent, |ui| {
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelThreads, lang));
            ui.add(egui::DragValue::new(&mut settings.threads).range(-1..=256));
            ui.small(i18n::t(i18n::Key::HintThreadsDefault, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpThreads, lang));
        });
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelThreadsBatch, lang));
            ui.add(egui::DragValue::new(&mut settings.threads_batch).range(-1..=256));
            ui.small(i18n::t(i18n::Key::HintThreadsBatchDefault, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpThreadsBatch, lang));
        });
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelNPredict, lang));
            ui.add(
                egui::DragValue::new(&mut settings.n_predict)
                    .range(-1..=65536)
                    .speed(128),
            );
            ui.small(i18n::t(i18n::Key::HintNPredictLimit, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpNPredict, lang));
        });
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelKeep, lang));
            ui.add(
                egui::DragValue::new(&mut settings.keep)
                    .range(0..=8192)
                    .speed(16),
            );
            ui.small(i18n::t(i18n::Key::HintKeepNone, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpKeep, lang));
        });
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelSeed, lang));
            ui.add(
                egui::DragValue::new(&mut settings.seed)
                    .range(-1..=i32::MAX as i64)
                    .speed(1),
            );
            ui.small(i18n::t(i18n::Key::HintSeedRandom, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpSeed, lang));
        });
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelMainGpu, lang));
            ui.add(egui::DragValue::new(&mut settings.main_gpu).range(0..=16));
            ui.small(i18n::t(i18n::Key::HintMainGpuFirst, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpMainGpu, lang));
        });
    });

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
                widgets::toggle(ui, &mut settings.enable_temperature, "", accent);
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
                widgets::toggle(ui, &mut settings.enable_top_p, "", accent);
            });
            // top_k
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelTopK, lang));
                ui.add(egui::DragValue::new(&mut settings.top_k).range(0..=1000));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpTopK, lang));
                widgets::toggle(ui, &mut settings.enable_top_k, "", accent);
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
                widgets::toggle(ui, &mut settings.enable_repeat_penalty, "", accent);
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
                widgets::toggle(ui, &mut settings.enable_presence_penalty, "", accent);
            });
        },
    );

    // ── 采样器扩展 ──
    widgets::card(
        ui,
        i18n::t(i18n::Key::SectionSamplers, lang),
        accent,
        |ui| {
            // Min-P
            ui.horizontal(|ui| {
                widgets::toggle(
                    ui,
                    &mut settings.enable_min_p,
                    i18n::t(i18n::Key::CheckboxMinP, lang),
                    accent,
                );
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpMinP, lang));
            });
            if settings.enable_min_p {
                ui.indent("min_p_opt", |ui| {
                    ui.horizontal(|ui| {
                        ui.label(i18n::t(i18n::Key::LabelMinP, lang));
                        ui.add(
                            egui::Slider::new(&mut settings.min_p, 0.0..=1.0)
                                .smallest_positive(0.01)
                                .custom_formatter(|v, _| format!("{:.3}", v)),
                        );
                        ui.label(format!("{:.3}", settings.min_p));
                    });
                });
            }
            // Top-N-Sigma
            ui.horizontal(|ui| {
                widgets::toggle(
                    ui,
                    &mut settings.enable_top_n_sigma,
                    i18n::t(i18n::Key::CheckboxTopNSigma, lang),
                    accent,
                );
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpTopNSigma, lang));
            });
            if settings.enable_top_n_sigma {
                ui.indent("top_n_sigma_opt", |ui| {
                    ui.horizontal(|ui| {
                        ui.label(i18n::t(i18n::Key::LabelTopNSigma, lang));
                        ui.add(
                            egui::Slider::new(&mut settings.top_n_sigma, 0.0..=3.0)
                                .smallest_positive(0.01)
                                .custom_formatter(|v, _| format!("{:.2}", v)),
                        );
                        ui.label(format!("{:.2}", settings.top_n_sigma));
                    });
                });
            }
            // XTC
            ui.horizontal(|ui| {
                widgets::toggle(
                    ui,
                    &mut settings.enable_xtc,
                    i18n::t(i18n::Key::CheckboxXtc, lang),
                    accent,
                );
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpXtc, lang));
            });
            if settings.enable_xtc {
                ui.indent("xtc_opt", |ui| {
                    ui.horizontal(|ui| {
                        ui.label(i18n::t(i18n::Key::LabelXtcProbability, lang));
                        ui.add(
                            egui::Slider::new(&mut settings.xtc_probability, 0.0..=1.0)
                                .smallest_positive(0.01)
                                .custom_formatter(|v, _| format!("{:.2}", v)),
                        );
                        ui.label(format!("{:.2}", settings.xtc_probability));
                    });
                    ui.horizontal(|ui| {
                        ui.label(i18n::t(i18n::Key::LabelXtcThreshold, lang));
                        ui.add(
                            egui::Slider::new(&mut settings.xtc_threshold, 0.0..=1.0)
                                .smallest_positive(0.01)
                                .custom_formatter(|v, _| format!("{:.2}", v)),
                        );
                        ui.label(format!("{:.2}", settings.xtc_threshold));
                    });
                });
            }
            // Typical-P
            ui.horizontal(|ui| {
                widgets::toggle(
                    ui,
                    &mut settings.enable_typical_p,
                    i18n::t(i18n::Key::CheckboxTypicalP, lang),
                    accent,
                );
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpTypicalP, lang));
            });
            if settings.enable_typical_p {
                ui.indent("typical_p_opt", |ui| {
                    ui.horizontal(|ui| {
                        ui.label(i18n::t(i18n::Key::LabelTypicalP, lang));
                        ui.add(
                            egui::Slider::new(&mut settings.typical_p, 0.0..=1.0)
                                .smallest_positive(0.01)
                                .custom_formatter(|v, _| format!("{:.2}", v)),
                        );
                        ui.label(format!("{:.2}", settings.typical_p));
                    });
                });
            }
            // Mirostat
            ui.horizontal(|ui| {
                widgets::toggle(
                    ui,
                    &mut settings.enable_mirostat,
                    i18n::t(i18n::Key::CheckboxMirostat, lang),
                    accent,
                );
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpMirostat, lang));
            });
            if settings.enable_mirostat {
                ui.indent("mirostat_opt", |ui| {
                    ui.horizontal(|ui| {
                        ui.label(i18n::t(i18n::Key::LabelMirostat, lang));
                        let m_vals = [0, 1, 2];
                        let m_labels = ["0 = 关", "1", "2"];
                        let mut m_idx = m_vals
                            .iter()
                            .position(|v| *v == settings.mirostat)
                            .unwrap_or(0);
                        widgets::segmented(ui, &m_labels, &mut m_idx, accent);
                        settings.mirostat = m_vals[m_idx];
                    });
                    ui.horizontal(|ui| {
                        ui.label(i18n::t(i18n::Key::LabelMirostatLr, lang));
                        ui.add(
                            egui::DragValue::new(&mut settings.mirostat_lr)
                                .range(0.0..=1.0)
                                .speed(0.01),
                        );
                        ui.label(format!("{:.2}", settings.mirostat_lr));
                    });
                    ui.horizontal(|ui| {
                        ui.label(i18n::t(i18n::Key::LabelMirostatEnt, lang));
                        ui.add(
                            egui::DragValue::new(&mut settings.mirostat_ent)
                                .range(0.0..=20.0)
                                .speed(0.1),
                        );
                        ui.label(format!("{:.2}", settings.mirostat_ent));
                    });
                });
            }
            // 动态温度
            ui.horizontal(|ui| {
                widgets::toggle(
                    ui,
                    &mut settings.enable_dynatemp,
                    i18n::t(i18n::Key::CheckboxDynatemp, lang),
                    accent,
                );
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpDynatemp, lang));
            });
            if settings.enable_dynatemp {
                ui.indent("dynatemp_opt", |ui| {
                    ui.horizontal(|ui| {
                        ui.label(i18n::t(i18n::Key::LabelDynatempRange, lang));
                        ui.add(
                            egui::DragValue::new(&mut settings.dynatemp_range)
                                .range(0.0..=1.0)
                                .speed(0.05),
                        );
                        ui.label(format!("{:.2}", settings.dynatemp_range));
                    });
                    ui.horizontal(|ui| {
                        ui.label(i18n::t(i18n::Key::LabelDynatempExp, lang));
                        ui.add(
                            egui::DragValue::new(&mut settings.dynatemp_exp)
                                .range(0.0..=2.0)
                                .speed(0.05),
                        );
                        ui.label(format!("{:.2}", settings.dynatemp_exp));
                    });
                });
            }
            // 采样器序列
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelSamplerSeq, lang));
                ui.text_edit_singleline(&mut settings.sampler_seq);
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpSamplerSeq, lang));
            });
        },
    );

    // ── 思考控制（Reasoning / Thinking）──
    widgets::card(
        ui,
        i18n::t(i18n::Key::SectionReasoning, lang),
        accent,
        |ui| {
            // 思考模式：标签 + ❓提示框 + 分段选择
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
            // 思考深度
            ui.label(i18n::t(i18n::Key::LabelReasoningEffort, lang));
            let efforts = [
                "default", "minimal", "low", "medium", "high", "xhigh", "max",
            ];
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
            // 思考输出格式
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
            // 保留思维轨迹
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
            // 思考预算
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
            // 预算耗尽提示
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelReasoningBudgetMessage, lang));
                ui.text_edit_singleline(&mut settings.reasoning_budget_message);
                helper::help_button_inline(
                    ui,
                    i18n::t(i18n::Key::HelpReasoningBudgetMessage, lang),
                );
            });
        },
    );

    // ── 聊天模板 ──
    widgets::card(
        ui,
        i18n::t(i18n::Key::SectionChatTemplate, lang),
        accent,
        |ui| {
            // Jinja 引擎开关：标签 + ❓提示框 + 开关
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::CheckboxJinja, lang));
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpJinja, lang));
                widgets::toggle(ui, &mut settings.jinja_enabled, "", accent);
            });
            // 外部模板文件
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
        },
    );

    // ── 结构化输出 ──
    widgets::card(
        ui,
        i18n::t(i18n::Key::SectionStructuredOutput, lang),
        accent,
        |ui| {
            // JSON Schema
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelJsonSchema, lang));
                ui.text_edit_singleline(&mut settings.json_schema);
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpJsonSchema, lang));
            });
            // Grammar
            ui.horizontal(|ui| {
                ui.label(i18n::t(i18n::Key::LabelGrammar, lang));
                ui.text_edit_singleline(&mut settings.grammar);
                helper::help_button_inline(ui, i18n::t(i18n::Key::HelpGrammar, lang));
            });
        },
    );

    // ── KV 缓存配置 ──
    widgets::card(ui, i18n::t(i18n::Key::SectionKvCache, lang), accent, |ui| {
        // 模型加载模式（新版 --load-mode；auto 时沿用下方旧版 --mmap/--mlock 开关）
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

        // 长上下文 / 提示缓存（标签 + ❓提示框 + 开关）
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::CheckboxCachePrompt, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpCachePrompt, lang));
            widgets::toggle(ui, &mut settings.cache_prompt, "", accent);
        });
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelCacheReuse, lang));
            ui.add(
                egui::DragValue::new(&mut settings.cache_reuse)
                    .range(0..=65536)
                    .speed(64),
            );
            ui.small(i18n::t(i18n::Key::HintCacheReuseDisabled, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpCacheReuse, lang));
        });
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::CheckboxContextShift, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpContextShift, lang));
            widgets::toggle(ui, &mut settings.context_shift, "", accent);
        });

        // KV 缓存开关统一样式（与「手动指定 GPU 层数」一致）：标签 + ❓提示框 + 开关
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::CheckboxKvOffload, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpKvOffload, lang));
            widgets::toggle(ui, &mut settings.kv_offload, "", accent);
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
            widgets::toggle(ui, &mut settings.kv_mlock, "", accent);
        });
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::CheckboxKvMmap, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpKvMmap, lang));
            widgets::toggle(ui, &mut settings.kv_mmap, "", accent);
        });
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::CheckboxKvUnified, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpKvUnified, lang));
            widgets::toggle(ui, &mut settings.kv_unified, "", accent);
        });

        // 完整滑动窗口 (--swa-full)，与「手动指定 GPU 层数」一致：标签 + ❓提示框 + 开关
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::CheckboxSwaFull, lang));
            helper::help_button_inline(ui, i18n::t(i18n::Key::HelpSwaFull, lang));
            widgets::toggle(ui, &mut settings.swa_full, "", accent);
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
                widgets::toggle(ui, &mut manual_gpu_layers, "", accent);
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
                widgets::toggle(ui, &mut settings.cpu_moe, "", accent);
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
