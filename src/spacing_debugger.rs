use egui::{pos2, vec2, Align2, Color32, Context, Painter, Rect, Response, Stroke, StrokeKind};

/// 调试结构体：记录控件矩形，检测悬浮，计算并绘制间距
pub struct SpacingDebugger {
    /// 本帧所有控件的屏幕矩形
    pub rects: Vec<Rect>,
}

impl SpacingDebugger {
    pub fn new() -> Self {
        Self { rects: Vec::new() }
    }

    /// 每帧开始时调用，清空上一帧的记录
    pub fn begin_frame(&mut self) {
        self.rects.clear();
    }

    #[allow(dead_code)]
    /// 包装一个控件，记录其矩形，并返回原 response
    /// 用法：let resp = debugger.record(ui.button("点击"));
    pub fn record(&mut self, response: Response) -> Response {
        self.rects.push(response.rect);
        response
    }

    /// 在所有控件绘制完成后调用，执行悬浮检测和可视化
    pub fn visualize(&self, ctx: &Context) {
        let pointer_pos = ctx.input(|i| i.pointer.hover_pos()).unwrap_or_default();

        // 找到鼠标悬浮的控件（取最后一个匹配的，即最上层）
        let hovered_rect = self
            .rects
            .iter()
            .rev()
            .find(|r| r.contains(pointer_pos))
            .copied();

        if let Some(hovered) = hovered_rect {
            // 高亮悬浮控件
            let painter = ctx.debug_painter();
            painter.rect_stroke(
                hovered,
                0.0,
                Stroke::new(2.0_f32, Color32::YELLOW),
                StrokeKind::Inside,
            );

            // 计算四个方向的间距并绘制
            self.draw_spacing(&painter, hovered, Direction::Up);
            self.draw_spacing(&painter, hovered, Direction::Down);
            self.draw_spacing(&painter, hovered, Direction::Left);
            self.draw_spacing(&painter, hovered, Direction::Right);
        }
    }

    fn draw_spacing(&self, painter: &Painter, hovered: Rect, dir: Direction) {
        // 在指定方向找到最近的邻居控件
        let neighbor = self
            .rects
            .iter()
            .filter(|r| **r != hovered) // 排除自身
            .filter(|r| match dir {
                Direction::Up => r.max.y <= hovered.min.y,    // 在上方
                Direction::Down => r.min.y >= hovered.max.y,  // 在下方
                Direction::Left => r.max.x <= hovered.min.x,  // 在左侧
                Direction::Right => r.min.x >= hovered.max.x, // 在右侧
            })
            .min_by_key(|r| match dir {
                Direction::Up | Direction::Down => (r.center().y - hovered.center().y).abs() as i32,
                Direction::Left | Direction::Right => {
                    (r.center().x - hovered.center().x).abs() as i32
                }
            });

        if let Some(neighbor) = neighbor {
            let (gap, line_start, line_end, label_pos, label) = match dir {
                Direction::Up => {
                    let gap = hovered.min.y - neighbor.max.y;
                    let x = hovered.center().x;
                    let start = pos2(x, neighbor.max.y);
                    let end = pos2(x, hovered.min.y);
                    let label_pos = pos2(x, (neighbor.max.y + hovered.min.y) / 2.0);
                    (gap, start, end, label_pos, format!("{:.0} px", gap))
                }
                Direction::Down => {
                    let gap = neighbor.min.y - hovered.max.y;
                    let x = hovered.center().x;
                    let start = pos2(x, hovered.max.y);
                    let end = pos2(x, neighbor.min.y);
                    let label_pos = pos2(x, (hovered.max.y + neighbor.min.y) / 2.0);
                    (gap, start, end, label_pos, format!("{:.0} px", gap))
                }
                Direction::Left => {
                    let gap = hovered.min.x - neighbor.max.x;
                    let y = hovered.center().y;
                    let start = pos2(neighbor.max.x, y);
                    let end = pos2(hovered.min.x, y);
                    let label_pos = pos2((neighbor.max.x + hovered.min.x) / 2.0, y);
                    (gap, start, end, label_pos, format!("{:.0} px", gap))
                }
                Direction::Right => {
                    let gap = neighbor.min.x - hovered.max.x;
                    let y = hovered.center().y;
                    let start = pos2(hovered.max.x, y);
                    let end = pos2(neighbor.min.x, y);
                    let label_pos = pos2((hovered.max.x + neighbor.min.x) / 2.0, y);
                    (gap, start, end, label_pos, format!("{:.0} px", gap))
                }
            };

            if gap > 0.0 {
                // 绘制间距标尺线
                painter.line_segment([line_start, line_end], Stroke::new(2.0_f32, Color32::RED));
                // 绘制间距数值
                painter.text(
                    label_pos + vec2(5.0, 0.0),
                    Align2::LEFT_CENTER,
                    label,
                    egui::FontId::proportional(12.0),
                    Color32::RED,
                );
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}
