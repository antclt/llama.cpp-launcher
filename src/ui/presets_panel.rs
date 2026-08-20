use crate::config::settings::{AppSettings, GpuLayersMode, Preset};
use crate::i18n;
use crate::ui::widgets;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// 导出/导入兼容旧文件的默认值（与 settings.rs 保持一致）
fn default_reasoning() -> String {
    "auto".to_string()
}
fn default_reasoning_format() -> String {
    "auto".to_string()
}
fn default_reasoning_effort() -> String {
    "default".to_string()
}
fn default_reasoning_budget() -> i32 {
    -1
}
fn default_jinja_enabled() -> bool {
    true
}

/// 导出/导入的“参数面板”专用结构（不包含 Server/RPC/模型路径等）
#[derive(Serialize, Deserialize)]
struct ParamsExport {
    context: usize,
    batch_size: usize,
    ubatch_size: f32,
    temperature: f32,
    top_p: f32,
    top_k: i32,
    repeat_penalty: f32,
    presence_penalty: f32,
    enable_temperature: bool,
    enable_top_p: bool,
    enable_top_k: bool,
    enable_repeat_penalty: bool,
    enable_presence_penalty: bool,
    flash_attn: String,

    spec_type: String,
    spec_draft_n_max: usize,
    spec_draft_n_min: usize,
    spec_draft_p_min: f32,
    spec_draft_p_split: f32,

    kv_offload: bool,
    cache_type_k: String,
    cache_type_v: String,
    kv_mlock: bool,
    kv_mmap: bool,
    kv_unified: bool,
    swa_full: bool,

    gpu_layers_mode: GpuLayersMode,
    split_mode: String,
    tensor_split: String,
    cpu_moe: bool,
    n_cpu_moe: usize,

    // 思考与会话
    #[serde(default = "default_reasoning")]
    reasoning: String,
    #[serde(default = "default_reasoning_format")]
    reasoning_format: String,
    #[serde(default = "default_reasoning_effort")]
    reasoning_effort: String,
    #[serde(default = "default_reasoning_budget")]
    reasoning_budget: i32,
    #[serde(default = "default_jinja_enabled")]
    jinja_enabled: bool,
    #[serde(default)]
    chat_template: String,
    #[serde(default)]
    chat_template_file: PathBuf,
}

impl ParamsExport {
    fn from_settings(s: &AppSettings) -> Self {
        Self {
            context: s.context,
            batch_size: s.batch_size,
            ubatch_size: s.ubatch_size,
            temperature: s.temperature,
            top_p: s.top_p,
            top_k: s.top_k,
            repeat_penalty: s.repeat_penalty,
            presence_penalty: s.presence_penalty,
            enable_temperature: s.enable_temperature,
            enable_top_p: s.enable_top_p,
            enable_top_k: s.enable_top_k,
            enable_repeat_penalty: s.enable_repeat_penalty,
            enable_presence_penalty: s.enable_presence_penalty,
            flash_attn: s.flash_attn.clone(),

            spec_type: s.spec_type.clone(),
            spec_draft_n_max: s.spec_draft_n_max,
            spec_draft_n_min: s.spec_draft_n_min,
            spec_draft_p_min: s.spec_draft_p_min,
            spec_draft_p_split: s.spec_draft_p_split,

            kv_offload: s.kv_offload,
            cache_type_k: s.cache_type_k.clone(),
            cache_type_v: s.cache_type_v.clone(),
            kv_mlock: s.kv_mlock,
            kv_mmap: s.kv_mmap,
            kv_unified: s.kv_unified,
            swa_full: s.swa_full,

            gpu_layers_mode: s.gpu_layers_mode,
            split_mode: s.split_mode.clone(),
            tensor_split: s.tensor_split.clone(),
            cpu_moe: s.cpu_moe,
            n_cpu_moe: s.n_cpu_moe,
            reasoning: s.reasoning.clone(),
            reasoning_format: s.reasoning_format.clone(),
            reasoning_effort: s.reasoning_effort.clone(),
            reasoning_budget: s.reasoning_budget,
            jinja_enabled: s.jinja_enabled,
            chat_template: s.chat_template.clone(),
            chat_template_file: s.chat_template_file.clone(),
        }
    }

    fn apply_to(self, s: &mut AppSettings) {
        s.context = self.context;
        s.batch_size = self.batch_size;
        s.ubatch_size = self.ubatch_size;
        s.temperature = self.temperature;
        s.top_p = self.top_p;
        s.top_k = self.top_k;
        s.repeat_penalty = self.repeat_penalty;
        s.presence_penalty = self.presence_penalty;
        s.enable_temperature = self.enable_temperature;
        s.enable_top_p = self.enable_top_p;
        s.enable_top_k = self.enable_top_k;
        s.enable_repeat_penalty = self.enable_repeat_penalty;
        s.enable_presence_penalty = self.enable_presence_penalty;
        s.flash_attn = self.flash_attn;

        s.spec_type = self.spec_type;
        s.spec_draft_n_max = self.spec_draft_n_max;
        s.spec_draft_n_min = self.spec_draft_n_min;
        s.spec_draft_p_min = self.spec_draft_p_min;
        s.spec_draft_p_split = self.spec_draft_p_split;

        s.kv_offload = self.kv_offload;
        s.cache_type_k = self.cache_type_k;
        s.cache_type_v = self.cache_type_v;
        s.kv_mlock = self.kv_mlock;
        s.kv_mmap = self.kv_mmap;
        s.kv_unified = self.kv_unified;
        s.swa_full = self.swa_full;

        s.gpu_layers_mode = self.gpu_layers_mode;
        s.split_mode = self.split_mode;
        s.tensor_split = self.tensor_split;
        s.cpu_moe = self.cpu_moe;
        s.n_cpu_moe = self.n_cpu_moe;
        s.reasoning = self.reasoning;
        s.reasoning_format = self.reasoning_format;
        s.reasoning_effort = self.reasoning_effort;
        s.reasoning_budget = self.reasoning_budget;
        s.jinja_enabled = self.jinja_enabled;
        s.chat_template = self.chat_template;
        s.chat_template_file = self.chat_template_file;
    }
}

pub fn ui(ui: &mut egui::Ui, settings: &mut AppSettings, lang: &i18n::Language) -> bool {
    let accent = crate::theme::accent_color(&settings.accent_color);
    let mut should_start_server = false;

    widgets::card(ui, i18n::t(i18n::Key::SectionPresets, lang), accent, |ui| {
        // 保存预设区域
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelPresetName, lang));
            ui.text_edit_singleline(&mut settings.new_preset_name);
            if ui.button(i18n::t(i18n::Key::BtnSavePreset, lang)).clicked() {
                let trimmed = settings.new_preset_name.trim().to_string();
                if !trimmed.is_empty() {
                    let exists = settings.presets.iter().any(|p| p.name == trimmed);
                    if !exists {
                        let preset = Preset::from_settings(settings, trimmed);
                        settings.presets.push(preset);
                    } else {
                        if let Some(idx) = settings.presets.iter().position(|p| p.name == trimmed) {
                            let new_preset = Preset::from_settings(settings, trimmed);
                            settings.presets[idx] = new_preset;
                        }
                    }
                    settings.new_preset_name.clear();
                }
            }

            if ui
                .small_button(i18n::t(i18n::Key::BtnExportParams, lang))
                .clicked()
            {
                let params = ParamsExport::from_settings(settings);
                let json = match serde_json::to_string_pretty(&params) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("[导出参数预设] 序列化失败: {}", e);
                        return;
                    }
                };
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name("llama_cpp_launcher_parameter_export.json")
                    .add_filter("JSON", &["json"])
                    .save_file()
                {
                    if let Err(e) = std::fs::write(&path, &json) {
                        eprintln!("[导出参数预设] 写入失败: {}", e);
                    }
                }
            }

            if ui
                .small_button(i18n::t(i18n::Key::BtnImportParams, lang))
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("JSON", &["json"])
                    .pick_file()
                {
                    let content = match std::fs::read_to_string(&path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("[导入参数预设] 读取失败: {}", e);
                            return;
                        }
                    };
                    match serde_json::from_str::<ParamsExport>(&content) {
                        Ok(params) => {
                            params.apply_to(settings);
                        }
                        Err(e) => {
                            eprintln!("[导入参数预设] 解析失败: {}", e);
                        }
                    }
                }
            }
        });

        ui.separator();

        if settings.presets.is_empty() {
            // 固定 200px 高的居中区域：避免 centered_and_justified 撑满整个
            // ScrollArea 内容区（content == inner 临界触发外层滚动条），
            // 同时保留提示文字垂直居中的视觉。
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 545.0),
                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                |ui| {
                    ui.label(i18n::t(i18n::Key::HintNoPresets, lang));
                },
            );
        } else {
            // 与日志面板相同的余量处理：列表超高时 ScrollArea 高度最多到
            // 剩余空间-18px，避免 card 总高精确顶满外层 ScrollArea 的 inner
            // 边界（临界触发外层滚动条，与内层滚动条叠加）；列表矮时
            // auto_shrink 默认收缩，布局不受影响。
            let max_h = (ui.available_height() - 18.0).max(64.0);
            egui::ScrollArea::vertical()
                .max_height(max_h)
                .show(ui, |ui| {
                    let mut load_index: Option<usize> = None;
                    let mut delete_index: Option<usize> = None;
                    let mut auto_start_preset: Option<String> = None;

                    for (i, preset) in settings.presets.iter().enumerate() {
                        ui.horizontal(|ui| {
                            if ui
                                .selectable_label(false, format!("📦 {}", preset.name))
                                .clicked()
                            {
                                load_index = Some(i);
                            }

                            if ui
                                .small_button(i18n::t(i18n::Key::BtnApplyPreset, lang))
                                .clicked()
                            {
                                load_index = Some(i);
                            }

                            let mut is_auto = settings
                                .auto_start_preset_name
                                .as_ref()
                                .is_some_and(|name| *name == preset.name);
                            if ui
                                .checkbox(
                                    &mut is_auto,
                                    i18n::t(i18n::Key::CheckboxAutoStartPreset, lang),
                                )
                                .changed()
                            {
                                if is_auto {
                                    auto_start_preset = Some(preset.name.clone());
                                } else if settings.auto_start_preset_name.as_ref()
                                    == Some(&preset.name)
                                {
                                    settings.auto_start_preset_name = None;
                                }
                            }

                            if ui
                                .small_button(i18n::t(i18n::Key::BtnRenamePreset, lang))
                                .clicked()
                            {
                                settings.rename_preset_index = Some(i);
                                settings.rename_preset_new_name = preset.name.clone();
                            }

                            if ui
                                .small_button(i18n::t(i18n::Key::BtnDeletePreset, lang))
                                .clicked()
                            {
                                delete_index = Some(i);
                            }
                        });
                        ui.separator();
                    }

                    if let Some(name) = auto_start_preset {
                        settings.auto_start_preset_name = Some(name);
                    }

                    if let Some(idx) = load_index {
                        if idx < settings.presets.len() {
                            let preset = settings.presets[idx].clone();
                            preset.apply_to(settings);
                        }
                    }

                    should_start_server = load_index
                        .map(|idx| {
                            idx < settings.presets.len()
                                && settings.auto_start_preset_name.as_ref()
                                    == Some(&settings.presets[idx].name)
                        })
                        .unwrap_or(false);

                    if let Some(idx) = delete_index {
                        if idx < settings.presets.len() {
                            settings.presets.remove(idx);
                            if let Some(rename_idx) = settings.rename_preset_index {
                                if rename_idx >= settings.presets.len() {
                                    settings.rename_preset_index = None;
                                    settings.rename_preset_new_name.clear();
                                }
                            }
                        }
                    }

                    if let Some(idx) = settings.rename_preset_index {
                        if idx < settings.presets.len() {
                            egui::Window::new(i18n::t(i18n::Key::BtnRenamePreset, lang))
                                .collapsible(false)
                                .resizable(false)
                                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                                .fixed_size([150.0, 0.0])
                                .show(ui.ctx(), |ui| {
                                    ui.label(i18n::t(i18n::Key::LabelPresetName, lang));
                                    ui.text_edit_singleline(&mut settings.rename_preset_new_name);
                                    ui.horizontal(|ui| {
                                        if ui.button("确认").clicked() {
                                            let trimmed =
                                                settings.rename_preset_new_name.trim().to_string();
                                            if !trimmed.is_empty() {
                                                settings.presets[idx].name = trimmed;
                                            }
                                            settings.rename_preset_index = None;
                                            settings.rename_preset_new_name.clear();
                                        }
                                        if ui.button("取消").clicked() {
                                            settings.rename_preset_index = None;
                                            settings.rename_preset_new_name.clear();
                                        }
                                    });
                                });
                        }
                    }
                });
        }
    });

    should_start_server
}
