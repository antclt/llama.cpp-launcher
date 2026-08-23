use crate::config::settings::AppSettings;
use crate::i18n;
use crate::ui::widgets;
use poll_promise::Promise;
use std::cell::RefCell;

// 跨帧持有的异步加载任务（Promise 不实现 Clone/Sync，无法存入 egui 临时数据）
thread_local! {
    static DETAIL_PROMISE: RefCell<Option<Promise<Result<ModelDetails, String>>>> = const { RefCell::new(None) };
}

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
    if s.starts_with("iq") && s.chars().nth(2).is_some_and(|c| c.is_ascii_digit()) {
        return true;
    }
    s.starts_with('q') && s.chars().nth(1).is_some_and(|c| c.is_ascii_digit())
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
    lower.contains("mmproj")
        || lower.contains("clip")
        || (lower.contains("proj") && lower.contains("vision"))
}

fn is_dflash_file(filename: &str) -> bool {
    filename.to_lowercase().contains("dflash")
}

/// 递归收集模型文件。选中的目录及其所有子目录都会被扫描，
/// 并跳过符号链接目录，避免循环引用导致界面卡住。
fn collect_model_files(dir: &std::path::Path, mode: FileMode) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
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
        let name = entry
            .file_name()
            .to_string_lossy()
            .to_string()
            .to_lowercase();
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

/// 模型详情（点击 📋 按钮后读取的 GGUF 元数据）
#[derive(Debug, Clone, Default)]
struct ModelDetails {
    /// 文件名
    file_name: String,
    /// 文件大小（字节）
    file_size: u64,
    /// 架构 (general.architecture)
    architecture: String,
    /// 参数量（人类可读，如 "27B"）
    parameters: String,
    /// 量化类型（人类可读，如 "Mostly Q4_0"）
    quantization: String,
    /// 上下文长度（若存在）
    context_length: Option<u64>,
    /// 嵌入维度（若存在）
    embedding_length: Option<u64>,
    /// 层数（若存在）
    block_count: Option<u64>,
    /// 张量数量
    tensor_count: u64,
}

/// 模型详情弹窗的跨帧状态（存于 Ui 临时数据，按钮点击后保持打开）
#[derive(Clone, Default)]
struct DetailsPopupState {
    /// 弹窗是否打开
    open: bool,
    /// 弹窗展示的详情（异步加载完成前为 None）
    details: Option<ModelDetails>,
}

/// 读取 GGUF 模型详情（流式读取文件头元数据，不加载张量数据）
fn load_model_details(
    file_path: &std::path::Path,
    lang: &i18n::Language,
) -> Result<ModelDetails, String> {
    let file_name = file_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let file_size = std::fs::metadata(file_path)
        .map(|m| m.len())
        .map_err(|e| format!("{}: {}", i18n::t(i18n::Key::ModelDetailsError, lang), e))?;

    let file_str = file_path
        .to_str()
        .ok_or_else(|| i18n::t(i18n::Key::ModelDetailsError, lang).to_string())?;

    let mut container = gguf_rs::get_gguf_container(file_str)
        .map_err(|e| format!("{}: {}", i18n::t(i18n::Key::ModelDetailsError, lang), e))?;

    let model = container
        .decode()
        .map_err(|e| format!("{}: {}", i18n::t(i18n::Key::ModelDetailsError, lang), e))?;

    let kv = model.metadata();
    let architecture = model.model_family();

    // 上下文长度（general.context_length → {arch}.context_length）
    let context_length = kv
        .get("general.context_length")
        .or_else(|| kv.get(&format!("{}.context_length", architecture)))
        .and_then(|v| v.as_u64());

    // 嵌入维度
    let embedding_length = kv
        .get(&format!("{}.embedding_length", architecture))
        .and_then(|v| v.as_u64());

    // 层数
    let block_count = kv
        .get(&format!("{}.block_count", architecture))
        .and_then(|v| v.as_u64());

    Ok(ModelDetails {
        file_name,
        file_size,
        architecture,
        parameters: model.model_parameters(),
        quantization: model.file_type(),
        context_length,
        embedding_length,
        block_count,
        tensor_count: model.num_tensor(),
    })
}

/// 人类可读的文件大小
fn format_file_size(bytes: u64) -> String {
    const UNITS: [&str; 3] = ["MB", "GB", "TB"];
    let mut size = bytes as f64 / (1024.0 * 1024.0);
    let mut unit = 0usize;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if size >= 100.0 {
        format!("{:.0} {}", size, UNITS[unit])
    } else {
        format!("{:.1} {}", size, UNITS[unit])
    }
}

/// 详情弹窗中的一行：标签 + 加粗值（明亮主题下值使用纯黑色）
fn detail_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).color(ui.visuals().weak_text_color()));
        let mut text = egui::RichText::new(value).strong();
        // 明亮主题下显式使用纯黑色，让具体值更醒目
        if !ui.visuals().dark_mode {
            text = text.color(egui::Color32::BLACK);
        }
        ui.label(text);
    });
}

fn render_file_list(
    ui: &mut egui::Ui,
    dir: &std::path::Path,
    selected_path: std::path::PathBuf,
    on_select: &mut impl FnMut(std::path::PathBuf),
    on_show_details: &mut impl FnMut(std::path::PathBuf),
    lang: &i18n::Language,
    mode: FileMode,
    accent: egui::Color32,
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

    let scroll_id = match mode {
        FileMode::Main => "model_scroll_main",
        FileMode::Mmproj => "model_scroll_mmproj",
        FileMode::Dflash => "model_scroll_dflash",
    };
    egui::ScrollArea::horizontal()
        .id_salt(scroll_id)
        .show(ui, |ui| {
            // 禁用系统自动行距，全部用显式间距控制分组关系：
            //   上一模型 ──GROUP_GAP──▶ 文件夹标题 ──HEADER_GAP──▶ 下属模型 ──ITEM_GAP──▶ …
            const GROUP_GAP: f32 = 16.0; // 文件夹分组之间的间距（模型 ↔ 文件夹标题）
            const HEADER_GAP: f32 = 6.0; // 文件夹标题与其下属首个模型之间的间距
            const ITEM_GAP: f32 = 8.0; // 同文件夹内模型之间的间距
            const HEADER_LINE_LEN: f32 = 24.0; // 文件夹标题后的短分隔线长度
            ui.spacing_mut().item_spacing.y = 0.0;
            let mut last_parent_dir: Option<std::path::PathBuf> = None;
            let mut first_item = true;
            let mut pending_space = 0.0_f32;
            for entry in entries {
                let file_path = entry.clone();
                let filename = file_path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default();
                let selected = selected_path == file_path;

                // 若模型位于子文件夹且为新的文件夹，先输出文件夹标题（主题色提亮文字 +
                // 短横线分区）
                let parent_dir = file_path
                    .parent()
                    .filter(|p| *p != dir)
                    .map(ToOwned::to_owned);
                let parent_changed = parent_dir.as_ref() != last_parent_dir.as_ref();
                if let Some(ref pdir) = parent_dir {
                    if parent_changed {
                        last_parent_dir = Some(pdir.clone());
                        if !first_item && pending_space > 0.0 {
                            ui.add_space(GROUP_GAP); // 标题前：与上一个模型拉开分组间距
                        }
                        // 文件夹标题：比主题色提亮一档（加白 25%），柔和不刺眼；
                        // 文字后跟一条短横线作分组连接符
                        let header_color = crate::theme::lighten_accent(accent, 0.25);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(
                                    pdir.file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_default(),
                                )
                                .color(header_color),
                            );
                            // 短分隔线：与主题色同色、较淡，垂直居中于标题文字
                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(HEADER_LINE_LEN, 2.0),
                                egui::Sense::hover(),
                            );
                            let y = rect.center().y;
                            ui.painter().line_segment(
                                [
                                    egui::Pos2::new(rect.left(), y),
                                    egui::Pos2::new(rect.right(), y),
                                ],
                                egui::Stroke::new(1.0_f32, header_color.gamma_multiply(0.6_f32)),
                            );
                        });
                        pending_space = HEADER_GAP; // 标题后：与下属首个模型贴近
                    }
                } else {
                    last_parent_dir = None;
                }

                // 模型行前间距
                if pending_space > 0.0 {
                    ui.add_space(pending_space);
                }

                ui.horizontal(|ui| {
                    if ui.add(egui::RadioButton::new(selected, "")).clicked() {
                        on_select(file_path.clone());
                    }
                    let tags = parse_tags(&filename);
                    for (text, color) in &tags {
                        ui.add(
                            egui::Button::new(
                                egui::RichText::new(text).color(widgets::contrast_text(*color)),
                            )
                            .fill(*color)
                            .corner_radius(4.0),
                        );
                    }
                    ui.separator();
                    let relative = file_path
                        .strip_prefix(dir)
                        .unwrap_or(&file_path)
                        .to_string_lossy();
                    ui.label(
                        egui::RichText::new(relative.as_ref())
                            .color(ui.visuals().weak_text_color()),
                    );
                    // 模型详情按钮
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new("📋").size(12.0))
                                .fill(egui::Color32::TRANSPARENT)
                                .corner_radius(4.0),
                        )
                        .on_hover_text(i18n::t(i18n::Key::BtnShowDetails, lang))
                        .clicked()
                    {
                        on_show_details(file_path.clone());
                    }
                });
                first_item = false;
                pending_space = ITEM_GAP; // 模型后：与下一个模型/标题分隔
            }
        });
}

pub fn ui(ui: &mut egui::Ui, settings: &mut AppSettings, lang: &i18n::Language) {
    let accent = crate::theme::accent_color(&settings.accent_color);

    // 模型详情弹窗跨帧状态（Ui 临时数据）
    let popup_id = ui.id().with("model_details_popup");
    // 本帧由详情按钮点击产生的异步加载任务
    let mut pending_promise: Option<Promise<Result<ModelDetails, String>>> = None;

    // ── 模型文件夹 ──
    widgets::card(
        ui,
        i18n::t(i18n::Key::PanelModelTitle, lang),
        accent,
        |ui| {
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
        },
    );

    // 未选择模型文件夹时，下方区域留空（不显示提示文本）
    if settings.model_dir.as_os_str().is_empty() {
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
            &mut |path| {
                let lang = lang.clone();
                let promise = Promise::spawn_thread("load_model_details", move || {
                    load_model_details(&path, &lang)
                });
                pending_promise = Some(promise);
            },
            lang,
            FileMode::Main,
            accent,
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
            &mut |path| {
                let lang = lang.clone();
                let promise = Promise::spawn_thread("load_model_details", move || {
                    load_model_details(&path, &lang)
                });
                pending_promise = Some(promise);
            },
            lang,
            FileMode::Mmproj,
            accent,
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
            &mut |path| {
                let lang = lang.clone();
                let promise = Promise::spawn_thread("load_model_details", move || {
                    load_model_details(&path, &lang)
                });
                pending_promise = Some(promise);
            },
            lang,
            FileMode::Dflash,
            accent,
        );
    });

    // ── 模型详情弹窗 ──
    // 本帧点击了详情按钮：将异步任务存入 thread_local，确保弹窗打开
    if let Some(promise) = pending_promise.take() {
        DETAIL_PROMISE.with(|cell| {
            *cell.borrow_mut() = Some(promise);
        });
        // 若弹窗尚未打开，初始化一个空状态
        ui.data_mut(|map| {
            map.insert_temp(
                popup_id,
                DetailsPopupState {
                    open: true,
                    details: None,
                },
            );
        });
    }

    // 轮询 thread_local 中的异步任务，完成后写入弹窗状态
    DETAIL_PROMISE.with(|cell| {
        let mut guard = cell.borrow_mut();
        if let Some(ref promise) = *guard {
            if let Some(result) = promise.ready() {
                match result {
                    Ok(details) => {
                        ui.data_mut(|map| {
                            if let Some(mut state) = map.get_temp::<DetailsPopupState>(popup_id) {
                                state.details = Some(details.clone());
                                map.insert_temp(popup_id, state);
                            }
                        });
                    }
                    Err(e) => {
                        log::error!(
                            "[model_panel] {}: {}",
                            i18n::t(i18n::Key::ModelDetailsError, lang),
                            e
                        );
                    }
                }
                *guard = None;
            }
        }
    });

    // 读取弹窗状态（跨帧保持打开，直到用户关闭）
    let mut popup = ui.data(|map| map.get_temp::<DetailsPopupState>(popup_id));
    if let Some(ref mut state) = popup {
        if state.open {
            // 用局部变量承载开关状态，避免窗口构建器与内容闭包的借用冲突
            let mut open = state.open;
            let ctx = ui.ctx();
            egui::Window::new(i18n::t(i18n::Key::DetailsWindowTitle, lang))
                .open(&mut open)
                .collapsible(true)
                .resizable(false)
                .min_size(egui::vec2(340.0, 0.0))
                .show(ctx, |ui| {
                    // 显示详情或加载状态
                    if let Some(ref details) = state.details {
                        let unknown = i18n::t(i18n::Key::DetailsUnknown, lang).to_string();

                        detail_row(
                            ui,
                            i18n::t(i18n::Key::DetailsFileName, lang),
                            &details.file_name,
                        );
                        detail_row(
                            ui,
                            i18n::t(i18n::Key::DetailsFileSize, lang),
                            &format_file_size(details.file_size),
                        );
                        ui.separator();
                        detail_row(
                            ui,
                            i18n::t(i18n::Key::DetailsArchitecture, lang),
                            &details.architecture,
                        );
                        detail_row(
                            ui,
                            i18n::t(i18n::Key::DetailsParameters, lang),
                            &details.parameters,
                        );
                        detail_row(
                            ui,
                            i18n::t(i18n::Key::DetailsQuantization, lang),
                            &details.quantization,
                        );
                        detail_row(
                            ui,
                            i18n::t(i18n::Key::DetailsContextLength, lang),
                            &details
                                .context_length
                                .map(|v| v.to_string())
                                .unwrap_or(unknown.clone()),
                        );
                        detail_row(
                            ui,
                            i18n::t(i18n::Key::DetailsEmbeddingLength, lang),
                            &details
                                .embedding_length
                                .map(|v| v.to_string())
                                .unwrap_or(unknown.clone()),
                        );
                        detail_row(
                            ui,
                            i18n::t(i18n::Key::DetailsBlockCount, lang),
                            &details
                                .block_count
                                .map(|v| v.to_string())
                                .unwrap_or(unknown.clone()),
                        );
                        detail_row(
                            ui,
                            i18n::t(i18n::Key::DetailsTensorCount, lang),
                            &details.tensor_count.to_string(),
                        );
                    } else {
                        // 异步任务进行中，显示加载指示器
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(i18n::t(i18n::Key::Loading, lang));
                        });
                    }
                });
            // 写回开关状态（用户可能已点击关闭按钮）
            state.open = open;
        }
    }
    // 写回弹窗状态：打开则保留，关闭则清理
    match popup {
        Some(state) if state.open => {
            ui.data_mut(|map| map.insert_temp(popup_id, state));
        }
        Some(_) => {
            ui.data_mut(|map| {
                map.remove_temp::<DetailsPopupState>(popup_id);
            });
        }
        None => {}
    }
}
