use crate::config::settings::AppSettings;
use crate::i18n;
use crate::ui::widgets;

/// 自动检测模型文件夹
fn auto_detect_model_dir() -> Option<std::path::PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();

    let dirs: Vec<_> = match std::fs::read_dir(&exe_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect(),
        Err(_) => return None,
    };

    let models_dir = dirs.iter().find(|e| {
        e.file_name()
            .to_string_lossy()
            .to_lowercase()
            .eq_ignore_ascii_case("models")
    });
    if let Some(dir) = models_dir {
        return Some(dir.path());
    }

    let model_dir = dirs.iter().find(|e| {
        e.file_name()
            .to_string_lossy()
            .to_lowercase()
            .eq_ignore_ascii_case("model")
    });
    if let Some(dir) = model_dir {
        return Some(dir.path());
    }

    None
}

/// 文件名解析为彩色标签（9 色方案）
fn parse_tags(filename: &str) -> Vec<(String, egui::Color32)> {
    let stem = filename.strip_suffix(".gguf").unwrap_or(filename);

    let purple = egui::Color32::from_rgb(180, 120, 255); // 参数量
    let orange = egui::Color32::from_rgb(255, 165, 0); // 量化类型
    let gray = egui::Color32::from_rgb(160, 160, 160); // 版本号
    let green = egui::Color32::from_rgb(100, 200, 100); // 训练方法
    let blue = egui::Color32::from_rgb(100, 150, 255); // 模型名称 (兜底)
    let yellow = egui::Color32::from_rgb(255, 215, 0); // 精度
    let pink = egui::Color32::from_rgb(255, 100, 130); // LoRA/Adapter
    let brown = egui::Color32::from_rgb(205, 133, 63); // 上下文长度
    let cyan = egui::Color32::from_rgb(0, 210, 210); // 架构类型

    let mut tags = Vec::new();
    for part in stem.split('-') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }

        let lower = trimmed.to_lowercase();
        let color = if is_param_size(&lower) {
            purple
        } else if is_quantization(&lower) {
            orange
        } else if trimmed.chars().all(|c| c.is_ascii_digit() || c == '.') {
            gray
        } else if is_training_method(&lower) {
            green
        } else if lower.contains("fp16")
            || lower.contains("bf16")
            || lower.contains("f32")
            || lower.contains("fp8")
        {
            yellow
        } else if lower.contains("lora") || lower.contains("adapter") || lower.contains("delta") {
            pink
        } else if is_context_length(&lower) {
            brown
        } else if lower.contains("mamba")
            || lower.contains("rwkv")
            || lower.contains("hyena")
            || lower.contains("decoder")
        {
            cyan
        } else {
            blue
        };

        tags.push((trimmed.to_string(), color));
    }

    tags
}

fn is_param_size(s: &str) -> bool {
    let has_digit = s.chars().any(|c| c.is_ascii_digit());
    has_digit && (s.ends_with('b') || s.ends_with('m'))
}

fn is_quantization(s: &str) -> bool {
    if s.starts_with("iq") && s.chars().nth(2).map_or(false, |c| c.is_ascii_digit()) {
        return true;
    }
    s.starts_with('q') && s.chars().nth(1).map_or(false, |c| c.is_ascii_digit())
}

fn is_training_method(s: &str) -> bool {
    s.contains("instruct")
        || s.contains("chat")
        || s.contains("sft")
        || s.contains("rlhf")
        || s.contains("dpo")
        || s.contains("orpo")
        || s.contains("grpo")
}

fn is_context_length(s: &str) -> bool {
    if s.ends_with('k') && s.contains(|c: char| c.is_ascii_digit()) {
        return true;
    }
    s.contains("long") || s == "128" || s == "64" || s == "32"
}

fn is_mmproj_file(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    lower.contains("mmproj") || lower.contains("clip") || (lower.contains("proj") && lower.contains("vision"))
}

fn is_dflash_file(filename: &str) -> bool {
    filename.to_lowercase().contains("dflash")
}

/// 递归收集模型文件。选中的目录及其所有子目录都会被扫描，
/// 并跳过符号链接目录，避免循环引用导致界面卡住。
fn collect_model_files(dir: &std::path::Path, mode: FileMode) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return files };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else { continue };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            files.extend(collect_model_files(&path, mode));
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string().to_lowercase();
        if !name.ends_with(".gguf") {
            continue;
        }
        let include = match mode {
            FileMode::Main => !is_mmproj_file(&name) && !is_dflash_file(&name),
            FileMode::Mmproj => is_mmproj_file(&name),
            FileMode::Dflash => is_dflash_file(&name),
        };
        if include {
            files.push(path);
        }
    }
    files
}

#[derive(Clone, Copy, PartialEq)]
enum FileMode {
    Main,
    Mmproj,
    Dflash,
}

fn render_file_list(
    ui: &mut egui::Ui,
    dir: &std::path::Path,
    selected_path: std::path::PathBuf,
    on_select: &mut impl FnMut(std::path::PathBuf),
    lang: &i18n::Language,
    mode: FileMode,
) {
    let mut entries = collect_model_files(dir, mode);
    entries.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));

    if entries.is_empty() {
        ui.colored_label(
            egui::Color32::GRAY,
            match mode {
                FileMode::Main => i18n::t(i18n::Key::NoGgufFiles, lang),
                FileMode::Mmproj => i18n::t(i18n::Key::NoMmprojFiles, lang),
                FileMode::Dflash => i18n::t(i18n::Key::NoDflashFiles, lang),
            },
        );
        return;
    }

    for entry in entries {
        let file_path = entry.clone();
        let filename = file_path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let selected = selected_path == file_path;

        ui.horizontal(|ui| {
            let tags = parse_tags(&filename);
            for (text, color) in &tags {
                ui.add(
                    egui::Button::new(egui::RichText::new(text).color(widgets::contrast_text(*color)))
                        .fill(*color)
                        .corner_radius(4.0),
                );
            }
            ui.separator();
            let relative = file_path
                .strip_prefix(dir)
                .unwrap_or(&file_path)
                .to_string_lossy();
            ui.label(egui::RichText::new(relative.as_ref()).color(ui.visuals().weak_text_color()));
            if ui.add(egui::RadioButton::new(selected, "")).clicked() {
                on_select(file_path);
            }
        });
    }
}

pub fn ui(ui: &mut egui::Ui, settings: &mut AppSettings, lang: &i18n::Language) {
    let accent = crate::theme::accent_color(&settings.accent_color);

    // ── 模型文件夹 ──
    widgets::card(ui, i18n::t(i18n::Key::PanelModelTitle, lang), accent, |ui| {
        ui.horizontal(|ui| {
            ui.label(i18n::t(i18n::Key::LabelModelDir, lang));
            let mut dir_str = settings.model_dir.to_string_lossy().to_string();
            let response = ui.text_edit_singleline(&mut dir_str);
            if response.changed() {
                settings.model_dir = std::path::PathBuf::from(&dir_str);
            }
        });

        ui.horizontal(|ui| {
            if ui
                .button(i18n::t(i18n::Key::BtnSelectFolder, lang))
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title(i18n::t(i18n::Key::DialogSelectFolder, lang))
                    .pick_folder()
                {
                    settings.model_dir = path;
                }
            }
            if ui.button(i18n::t(i18n::Key::BtnAutoDetect, lang)).clicked() {
                if let Some(path) = auto_detect_model_dir() {
                    settings.model_dir = path;
                } else {
                    settings.model_dir = std::path::PathBuf::from("");
                }
            }
        });
    });

    if settings.model_dir.as_os_str().is_empty() {
        ui.colored_label(egui::Color32::GRAY, i18n::t(i18n::Key::NoModelDir, lang));
        return;
    }

    // ── 模型文件 ──
    widgets::card(ui, i18n::t(i18n::Key::SectionModels, lang), accent, |ui| {
        let selected_model = settings.model_path.clone();
        render_file_list(
            ui,
            &settings.model_dir,
            selected_model,
            &mut |path| {
                settings.model_path = path;
            },
            lang,
            FileMode::Main,
        );
    });

    // ── mmproj ──
    widgets::card(ui, i18n::t(i18n::Key::SectionMmproj, lang), accent, |ui| {
        let selected_mmproj = settings.mmproj_path.clone();
        render_file_list(
            ui,
            &settings.model_dir,
            selected_mmproj.clone(),
            &mut |path| {
                settings.mmproj_path = if selected_mmproj == path {
                    std::path::PathBuf::new()
                } else {
                    path
                };
            },
            lang,
            FileMode::Mmproj,
        );
    });

    // ── DFlash ──
    widgets::card(ui, i18n::t(i18n::Key::SectionDflash, lang), accent, |ui| {
        let selected_dflash = settings.dflash_path.clone();
        render_file_list(
            ui,
            &settings.model_dir,
            selected_dflash.clone(),
            &mut |path| {
                settings.dflash_path = if selected_dflash == path {
                    std::path::PathBuf::new()
                } else {
                    path
                };
            },
            lang,
            FileMode::Dflash,
        );
    });
}
