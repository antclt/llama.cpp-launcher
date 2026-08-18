use egui_thematic::ThemeConfig;

/// Fluent UI 3 浅色主题预设
///
/// 对齐 HTML 原型 :root 变量：
///   --bg-app:#F5F5F7 / --bg-sidebar:#ECECEE / --bg-surface:#FFFFFF
///   --accent:#007AFF / --border:#E3E3E8 / radius-ctl:6px（用户要求更方正）
pub fn fluent3_light_preset() -> ThemeConfig {
    ThemeConfig {
        name: "Fluent3 Light".to_string(),
        dark_mode: false,

        // ── 文本颜色 ──
        override_text_color: Some([29, 29, 31, 255]),           // #1D1D1F 深黑（浅色模式主文本）
        override_weak_text_color: Some([110, 110, 115, 255]),    // #6E6E73 次要文本
        override_hyperlink_color: Some([0, 122, 255, 255]),      // #007AFF

        // ── 背景色（层次：surface > sidebar > app） ──
        override_faint_bg_color: Some([245, 245, 247, 255]),    // #F5F5F7 (app bg)
        override_extreme_bg_color: Some([255, 255, 255, 255]),   // white
        override_code_bg_color: Some([245, 245, 247, 255]),      // #F5F5F7

        // ── 窗口 ──
        override_window_fill: Some([245, 245, 247, 255]),         // #F5F5F7 — 主内容区背景
        override_window_stroke_color: Some([227, 227, 232, 255]), // #E3E3E8
        override_window_stroke_width: Some(1.0),
        override_window_corner_radius: Some(6),                    // ★ 6px 更方正
        override_window_shadow_size: Some(8),

        // ── 面板（侧边栏用，与卡片同色）──
        override_panel_fill: Some([255, 255, 255, 255]),            // #FFFFFF — 同 card surface

        // ── 弹窗阴影 ──
        override_popup_shadow_size: Some(12),

        // ── 选区 ──
        override_selection_bg: Some([0, 122, 255, 255]),
        override_selection_stroke_color: Some([0, 113, 235, 255]),
        override_selection_stroke_width: Some(1.0),

        // ── 语义色（警告 / 错误）──
        override_warn_fg_color: Some([255, 185, 0, 255]),    // #FFB900
        override_error_fg_color: Some([209, 52, 56, 255]),   // #D13438

        // ── 非交互控件（卡片/标签/分隔线）──
        override_widget_noninteractive_bg_fill: Some([255, 255, 255, 255]),     // white (card surface)
        override_widget_noninteractive_weak_bg_fill: Some([245, 245, 247, 255]),
        override_widget_noninteractive_bg_stroke_color: Some([227, 227, 232, 255]), // #E3E3E8
        override_widget_noninteractive_bg_stroke_width: Some(1.0),
        override_widget_noninteractive_corner_radius: Some(6),                  // ★ 6px
        override_widget_noninteractive_fg_stroke_color: Some([29, 29, 31, 255]),
        override_widget_noninteractive_fg_stroke_width: Some(1.0),
        override_widget_noninteractive_expansion: Some(0.0),

        // ── 非活动控件（默认按钮/输入框）──
        // 注意：滑块轨道(rail)也用此色绘制，必须与卡片背景(noninteractive.bg_fill=white)有反差，
        // 否则白轨压白底完全隐形。按钮/输入框有描边定义轮廓，不受影响。
        override_widget_inactive_bg_fill: Some([240, 240, 242, 255]),   // #F0F0F2 — 浅灰，在白色卡片上可见
        override_widget_inactive_weak_bg_fill: Some([245, 245, 247, 255]),
        override_widget_inactive_bg_stroke_color: Some([210, 210, 215, 255]), // #D2D2D7 border-strong
        override_widget_inactive_bg_stroke_width: Some(1.0),
        override_widget_inactive_corner_radius: Some(6),                        // ★ 6px 圆角矩形
        override_widget_inactive_fg_stroke_color: Some([29, 29, 31, 255]),
        override_widget_inactive_fg_stroke_width: Some(1.0),
        override_widget_inactive_expansion: Some(0.0),

        // ── 悬停控件 ──
        override_widget_hovered_bg_fill: Some([240, 240, 242, 255]),  // #F0F0F2
        override_widget_hovered_weak_bg_fill: Some([245, 245, 247, 255]),
        override_widget_hovered_bg_stroke_color: Some([0, 122, 255, 255]),
        override_widget_hovered_bg_stroke_width: Some(1.0),
        override_widget_hovered_corner_radius: Some(6),
        override_widget_hovered_fg_stroke_color: Some([29, 29, 31, 255]),
        override_widget_hovered_fg_stroke_width: Some(1.0),
        override_widget_hovered_expansion: Some(0.0),

        // ── 活动/按下控件 ──
        override_widget_active_bg_fill: Some([0, 90, 158, 255]),
        override_widget_active_weak_bg_fill: Some([0, 122, 212, 255]),
        override_widget_active_bg_stroke_color: Some([0, 59, 106, 255]),
        override_widget_active_bg_stroke_width: Some(1.0),
        override_widget_active_corner_radius: Some(6),
        override_widget_active_fg_stroke_color: Some([255, 255, 255, 255]),
        override_widget_active_fg_stroke_width: Some(1.0),
        override_widget_active_expansion: Some(0.0),

        // ── 展开/打开控件 ──
        override_widget_open_bg_fill: Some([240, 240, 242, 255]),
        override_widget_open_weak_bg_fill: Some([245, 245, 247, 255]),
        override_widget_open_bg_stroke_color: Some([0, 122, 255, 255]),
        override_widget_open_bg_stroke_width: Some(1.0),
        override_widget_open_corner_radius: Some(6),
        override_widget_open_fg_stroke_color: Some([29, 29, 31, 255]),
        override_widget_open_fg_stroke_width: Some(1.0),
        override_widget_open_expansion: Some(0.0),

        // ── 其他 ──
        override_resize_corner_size: Some(6.0),
        override_text_cursor_width: Some(2.0),
        override_clip_rect_margin: Some(3.0),
        override_button_frame: Some(true),
        override_collapsing_header_frame: Some(false),
        override_indent_has_left_vline: Some(false),
        override_striped: Some(false),
        override_slider_trailing_fill: Some(false),
    }
}

/// Apple HIG 深色主题预设
///
/// 对齐 HTML [data-theme="dark"] 变量：
///   --bg-app:#1C1C1E / --bg-sidebar:#232325 / --bg-surface:#2C2C2E
///   --accent:#0A84FF / --border:#38383A / --text-primary:#F5F5F7
pub fn fluent3_dark_preset() -> ThemeConfig {
    let mut t = fluent3_light_preset();
    t.dark_mode = true;

    // ── 文本颜色 ──
    t.override_text_color = Some([245, 245, 247, 255]);            // #F5F5F7
    t.override_weak_text_color = Some([174, 174, 178, 255]);       // #AEAEB2

    // ── 背景层次（主内容区 > 侧边栏 > 卡片表面）──
    t.override_faint_bg_color = Some([28, 28, 30, 255]);           // #1C1C1E — 主内容区最深
    t.override_extreme_bg_color = Some([16, 16, 18, 255]);          // 近纯黑
    t.override_code_bg_color = Some([28, 28, 30, 255]);             // #1C1C1E

    // ── 窗口填充 = 主内容区背景（比侧边栏更深）──
    t.override_window_fill = Some([28, 28, 30, 255]);              // #1C1C1E
    t.override_window_stroke_color = Some([56, 56, 58, 255]);      // #38383A — 细分隔线
    t.override_window_corner_radius = Some(6);                       // ★ 6px
    t.override_window_shadow_size = Some(16);

    // ── 面板 = 侧边栏背景（与卡片同色，用户要求 sidebar ≡ card surface）──
    t.override_panel_fill = Some([44, 44, 46, 255]);              // #2C2C2E — 同 noninteractive.bg_fill

    // ── 弹窗阴影 ──
    t.override_popup_shadow_size = Some(20);

    // ── 语义色 ──
    t.override_warn_fg_color = Some([255, 214, 10, 255]);          // #FFD60A
    t.override_error_fg_color = Some([255, 69, 58, 255]);          // #FF453A

    // ── 非交互控件（卡片 surface = #2C2C2E）──
    t.override_widget_noninteractive_bg_fill = Some([44, 44, 46, 255]);       // #2C2C2E
    t.override_widget_noninteractive_weak_bg_fill = Some([28, 28, 30, 255]);
    t.override_widget_noninteractive_bg_stroke_color = Some([56, 56, 58, 255]); // #38383A
    t.override_widget_noninteractive_fg_stroke_color = Some([174, 174, 178, 255]);
    t.override_widget_noninteractive_corner_radius = Some(6);                  // ★ 6px

    // ── 非活动控件（输入框/按钮默认态）──
    // 滑块轨道用此色，必须与卡片背景(#2C2C2E)有反差，否则轨道隐形。
    t.override_widget_inactive_bg_fill = Some([58, 58, 60, 255]);             // #3A3A3C — 比卡片稍亮，滑块轨可见
    t.override_widget_inactive_weak_bg_fill = Some([54, 54, 60, 255]);
    t.override_widget_inactive_bg_stroke_color = Some([72, 72, 74, 255]);       // #48484A border-strong
    t.override_widget_inactive_corner_radius = Some(6);                         // ★ 6px 圆角矩形
    t.override_widget_inactive_fg_stroke_color = Some([205, 207, 215, 255]);

    // ── 悬停控件 ──
    t.override_widget_hovered_bg_fill = Some([58, 58, 60, 255]);              // #3A3A3C
    t.override_widget_hovered_weak_bg_fill = Some([50, 50, 56, 255]);
    t.override_widget_hovered_bg_stroke_color = Some([90, 90, 100, 255]);
    t.override_widget_hovered_fg_stroke_color = Some([245, 245, 247, 255]);
    t.override_widget_hovered_corner_radius = Some(6);

    // ── 活动/按下控件 ──
    t.override_widget_active_bg_fill = Some([80, 80, 90, 255]);
    t.override_widget_active_weak_bg_fill = Some([80, 80, 90, 255]);
    t.override_widget_active_bg_stroke_color = Some([100, 100, 110, 255]);
    t.override_widget_active_fg_stroke_color = Some([255, 255, 255, 255]);
    t.override_widget_active_corner_radius = Some(6);

    // ── 展开/打开控件 ──
    t.override_widget_open_bg_fill = Some([58, 58, 60, 255]);
    t.override_widget_open_weak_bg_fill = Some([50, 50, 56, 255]);
    t.override_widget_open_bg_stroke_color = Some([90, 90, 100, 255]);
    t.override_widget_open_fg_stroke_color = Some([245, 245, 247, 255]);
    t.override_widget_open_corner_radius = Some(6);

    t
}

/// 解析十六进制颜色（支持 #RRGGBB / RRGGBB），失败时回退到 #0A84FF
pub fn parse_hex(s: &str) -> [u8; 3] {
    let s = s.trim_start_matches('#');
    if s.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&s[0..2], 16),
            u8::from_str_radix(&s[2..4], 16),
            u8::from_str_radix(&s[4..6], 16),
        ) {
            return [r, g, b];
        }
    }
    [10, 132, 255] // #0A84FF Apple system blue
}

/// 解析配置中的主题色为 egui Color32
pub fn accent_color(s: &str) -> egui::Color32 {
    let c = parse_hex(s);
    egui::Color32::from_rgb(c[0], c[1], c[2])
}

/// 应用主题到全局视觉样式
///
/// - dark: 深色模式开关
/// - accent: 全局强调色 (RGB)，用于超链接 / 选区 / 悬停描边
pub fn apply_theme(ctx: &egui::Context, dark: bool, accent: [u8; 3]) {
    let mut t = if dark {
        fluent3_dark_preset()
    } else {
        fluent3_light_preset()
    };
    let a = [accent[0], accent[1], accent[2], 255u8];
    t.override_hyperlink_color = Some(a);
    t.override_selection_bg = Some([accent[0], accent[1], accent[2], 96u8]);
    t.override_selection_stroke_color = Some([accent[0], accent[1], accent[2], 220]);
    // 注意：不设置 widget_open/hovered stroke 为 accent 色！
    // 那会导致面板边框（顶栏/侧栏/中央区分隔线）染上主题色紫/蓝色。
    // 这些描边保持 preset 中的中性色（light: #D2D2D7, dark: #48484A）即可。

    let mut visuals = t.to_visuals();
    // 提示框使用更克制的阴影，避免问号帮助框出现大面积黑色晕影。
    visuals.popup_shadow = egui::epaint::Shadow {
        offset: [2, 3],
        blur: 4,
        spread: 0,
        color: egui::Color32::from_black_alpha(64),
    };

    // 覆盖全局样式：确保所有按钮和输入框使用 6px 圆角（非 egui 默认的全圆角）
    ctx.set_visuals(visuals);
}
