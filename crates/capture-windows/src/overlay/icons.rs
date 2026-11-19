use windows::{
    Win32::Graphics::Direct2D::{Common::*, *},
    core::Result,
};
use windows_numerics::Vector2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Icon {
    Select,
    Rect,
    Spotlight,
    Arrow,
    Freehand,
    Undo,
    Redo,
    Cancel,
    Confirm,
}

pub(crate) unsafe fn draw_icon(
    context: &ID2D1DeviceContext,
    factory: &ID2D1Factory,
    round_stroke: &ID2D1StrokeStyle,
    icon: Icon,
    center: (f32, f32),
    scale: f32,
    brush: &ID2D1SolidColorBrush,
) -> Result<()> {
    // SAFETY: Caller owns an active BeginDraw/EndDraw session on `context`.
    unsafe {
        let stroke = 1.45 * scale;
        let pt = |dx: f32, dy: f32| Vector2 {
            X: center.0 + dx * scale,
            Y: center.1 + dy * scale,
        };

        match icon {
            Icon::Select => {
                let path = factory.CreatePathGeometry()?;
                let sink = path.Open()?;
                sink.BeginFigure(pt(-4.0, -5.5), D2D1_FIGURE_BEGIN_FILLED);
                sink.AddLines(&[
                    pt(-4.0, 5.0),
                    pt(-1.0, 2.2),
                    pt(1.2, 6.0),
                    pt(2.8, 5.2),
                    pt(0.6, 1.5),
                    pt(4.5, 1.2),
                ]);
                sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                sink.Close()?;
                context.DrawGeometry(&path, brush, 1.2 * scale, Some(round_stroke));
            }
            Icon::Rect => {
                let r = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: center.0 - 5.5 * scale,
                        top: center.1 - 4.2 * scale,
                        right: center.0 + 5.5 * scale,
                        bottom: center.1 + 4.2 * scale,
                    },
                    radiusX: 1.5 * scale,
                    radiusY: 1.5 * scale,
                };
                context.DrawRoundedRectangle(&r, brush, stroke, Some(round_stroke));
            }
            Icon::Spotlight => {
                // Outer corner focus brackets + bright inner cutout rect.
                let o = 6.0;
                let arm = 2.6;
                for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                    context.DrawLine(
                        pt(sx * o, sy * (o - arm)),
                        pt(sx * o, sy * o),
                        brush,
                        1.25 * scale,
                        Some(round_stroke),
                    );
                    context.DrawLine(
                        pt(sx * o, sy * o),
                        pt(sx * (o - arm), sy * o),
                        brush,
                        1.25 * scale,
                        Some(round_stroke),
                    );
                }
                let inner = D2D_RECT_F {
                    left: center.0 - 3.0 * scale,
                    top: center.1 - 2.4 * scale,
                    right: center.0 + 3.0 * scale,
                    bottom: center.1 + 2.4 * scale,
                };
                context.FillRectangle(&inner, brush);
            }
            Icon::Arrow => {
                context.DrawLine(
                    pt(-5.0, 5.0),
                    pt(4.8, -4.8),
                    brush,
                    stroke,
                    Some(round_stroke),
                );
                context.DrawLine(
                    pt(0.2, -5.0),
                    pt(5.0, -5.0),
                    brush,
                    stroke,
                    Some(round_stroke),
                );
                context.DrawLine(
                    pt(5.0, -5.0),
                    pt(5.0, -0.2),
                    brush,
                    stroke,
                    Some(round_stroke),
                );
            }
            Icon::Freehand => {
                let path = factory.CreatePathGeometry()?;
                let sink = path.Open()?;
                sink.BeginFigure(pt(-5.5, 3.8), D2D1_FIGURE_BEGIN_HOLLOW);
                sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                    point1: pt(-2.2, -5.5),
                    point2: pt(1.5, 5.5),
                    point3: pt(5.5, -3.6),
                });
                sink.EndFigure(D2D1_FIGURE_END_OPEN);
                sink.Close()?;
                context.DrawGeometry(&path, brush, stroke, Some(round_stroke));
            }
            Icon::Undo => {
                let path = factory.CreatePathGeometry()?;
                let sink = path.Open()?;
                sink.BeginFigure(pt(-4.8, -1.0), D2D1_FIGURE_BEGIN_HOLLOW);
                sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                    point1: pt(0.5, -4.8),
                    point2: pt(5.5, -2.5),
                    point3: pt(4.5, 3.6),
                });
                sink.EndFigure(D2D1_FIGURE_END_OPEN);
                sink.Close()?;
                context.DrawGeometry(&path, brush, stroke, Some(round_stroke));
                context.DrawLine(
                    pt(-5.2, -1.0),
                    pt(-1.6, -3.5),
                    brush,
                    stroke,
                    Some(round_stroke),
                );
                context.DrawLine(
                    pt(-5.2, -1.0),
                    pt(-2.2, 2.2),
                    brush,
                    stroke,
                    Some(round_stroke),
                );
            }
            Icon::Redo => {
                let path = factory.CreatePathGeometry()?;
                let sink = path.Open()?;
                sink.BeginFigure(pt(4.8, -1.0), D2D1_FIGURE_BEGIN_HOLLOW);
                sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                    point1: pt(-0.5, -4.8),
                    point2: pt(-5.5, -2.5),
                    point3: pt(-4.5, 3.6),
                });
                sink.EndFigure(D2D1_FIGURE_END_OPEN);
                sink.Close()?;
                context.DrawGeometry(&path, brush, stroke, Some(round_stroke));
                context.DrawLine(
                    pt(5.2, -1.0),
                    pt(1.6, -3.5),
                    brush,
                    stroke,
                    Some(round_stroke),
                );
                context.DrawLine(
                    pt(5.2, -1.0),
                    pt(2.2, 2.2),
                    brush,
                    stroke,
                    Some(round_stroke),
                );
            }
            Icon::Cancel => {
                context.DrawLine(
                    pt(-4.0, -4.0),
                    pt(4.0, 4.0),
                    brush,
                    stroke,
                    Some(round_stroke),
                );
                context.DrawLine(
                    pt(4.0, -4.0),
                    pt(-4.0, 4.0),
                    brush,
                    stroke,
                    Some(round_stroke),
                );
            }
            Icon::Confirm => {
                let path = factory.CreatePathGeometry()?;
                let sink = path.Open()?;
                sink.BeginFigure(pt(-4.5, 0.3), D2D1_FIGURE_BEGIN_HOLLOW);
                sink.AddLines(&[pt(-1.3, 3.6), pt(4.8, -3.4)]);
                sink.EndFigure(D2D1_FIGURE_END_OPEN);
                sink.Close()?;
                context.DrawGeometry(&path, brush, 1.75 * scale, Some(round_stroke));
            }
        }
        Ok(())
    }
}
