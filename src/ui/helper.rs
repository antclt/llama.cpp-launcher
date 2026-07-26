/// 帮助按钮组件 - 问号气泡提示
use crate::ui::widgets;
use egui::{Color32, RichText, Stroke, Vec2};

/// 帮助按钮样式配置
pub struct HelpButtonStyle {
    /// 按钮背景色
    pub fill: Color32,
    /// 按钮边框色
    pub stroke_color: Color32,
    /// 按钮圆角
    pub corner_radius: f32,
    /// 按钮大小
    pub size: Vec2,
    /// 文字大小
    pub text_size: f32,
}

impl Default for HelpButtonStyle {
    fn default() -> Self {
        Self {
            fill: Color32::from_rgb(100, 150, 255),
            stroke_color: Color32::from_rgb(70, 130, 230),
            corner_radius: 12.0,
            size: Vec2::new(20.0, 20.0),
            text_size: 12.0,
        }
    }
}

/// 创建帮助按钮
///
/// # 参数
/// * `ui` - egui UI 上下文
/// * `help_text` - 悬停时显示的帮助文本
///
/// # 返回值
/// 返回 `egui::Response`，可以继续链式调用 `.on_hover_text()` 等方法
///
/// # 示例
/// ```rust
/// // 基础用法
/// helper::help_button(ui, "这是帮助信息");
///
/// // 带自定义样式
/// let style = HelpButtonStyle {
///     fill: Color32::from_rgb(50, 200, 100),
///     ..Default::default()
/// };
/// helper::help_button_with_style(ui, "自定义样式帮助", &style);
/// ```
pub fn help_button(ui: &mut egui::Ui, help_text: &str) -> egui::Response {
    help_button_with_style(ui, help_text, &HelpButtonStyle::default())
}

/// 创建带自定义样式的帮助按钮
pub fn help_button_with_style(
    ui: &mut egui::Ui,
    help_text: &str,
    style: &HelpButtonStyle,
) -> egui::Response {
    // 保存并临时修改 hover/active 状态的视觉效果，防止按钮变宽和点击动画
    let visuals = ui.visuals_mut();
    let old_hover_expansion = visuals.widgets.hovered.expansion;
    let old_hover_bg_stroke = visuals.widgets.hovered.bg_stroke;
    let old_active_expansion = visuals.widgets.active.expansion;
    let old_active_bg_stroke = visuals.widgets.active.bg_stroke;
    visuals.widgets.hovered.expansion = 0.0;
    visuals.widgets.hovered.bg_stroke = Stroke::NONE;
    visuals.widgets.active.expansion = 0.0;
    visuals.widgets.active.bg_stroke = Stroke::NONE;

    let response = ui.add(
        egui::Button::new(
            RichText::new("❓")
                .color(widgets::contrast_text(style.fill))
                .strong()
                .size(style.text_size),
        )
        .fill(style.fill)
        .corner_radius(style.corner_radius)
        .min_size(style.size)
        .stroke(Stroke::new(1.0_f32, style.stroke_color))
        .sense(egui::Sense::hover()), // 只响应悬停，不响应点击
    );

    // 恢复原来的设置
    let visuals = ui.visuals_mut();
    visuals.widgets.hovered.expansion = old_hover_expansion;
    visuals.widgets.hovered.bg_stroke = old_hover_bg_stroke;
    visuals.widgets.active.expansion = old_active_expansion;
    visuals.widgets.active.bg_stroke = old_active_bg_stroke;

    response.on_hover_text(help_text)
}

/// 创建小型帮助按钮（更紧凑的样式）
pub fn help_button_small(ui: &mut egui::Ui, help_text: &str) -> egui::Response {
    let style = HelpButtonStyle {
        size: Vec2::new(16.0, 16.0),
        text_size: 10.0,
        corner_radius: 8.0,
        ..Default::default()
    };
    help_button_with_style(ui, help_text, &style)
}

/// 创建内联帮助按钮（与文本对齐）
pub fn help_button_inline(ui: &mut egui::Ui, help_text: &str) -> egui::Response {
    let style = HelpButtonStyle {
        size: Vec2::new(14.0, 14.0),
        text_size: 9.0,
        corner_radius: 7.0,
        fill: Color32::from_rgb(120, 120, 120),
        stroke_color: Color32::from_rgb(100, 100, 100),
        ..Default::default()
    };
    help_button_with_style(ui, help_text, &style)
}
