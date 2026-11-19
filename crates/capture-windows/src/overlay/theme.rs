use super::annotations::InkColor;
use windows::{
    Win32::{
        Foundation::E_FAIL,
        Graphics::Direct2D::{Common::*, *},
    },
    core::{Error, Result},
};

pub(crate) fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

pub(crate) fn token_rgba(name: &str, white_scale: f32, alpha: f32) -> Result<D2D1_COLOR_F> {
    // Reuse the Ardot export rather than maintaining a separate native palette.
    let tokens = include_str!("../../../../apps/desktop/src/renderer/src/tokens.generated.css");
    let prefix = format!("--Lumiere-Dark-{name}: #");
    let value = tokens
        .lines()
        .find_map(|line| line.trim().strip_prefix(&prefix))
        .and_then(|value| value.strip_suffix(';'))
        .and_then(|value| u32::from_str_radix(value, 16).ok())
        .ok_or_else(|| Error::new(E_FAIL, "Missing Region design color"))?;
    Ok(D2D1_COLOR_F {
        r: srgb_to_linear(((value >> 16) & 255) as f32 / 255.0) * white_scale,
        g: srgb_to_linear(((value >> 8) & 255) as f32 / 255.0) * white_scale,
        b: srgb_to_linear((value & 255) as f32 / 255.0) * white_scale,
        a: alpha,
    })
}

/// Cached Direct2D brushes and stroke styles for overlay and export rendering.
pub(crate) struct RenderResources {
    pub round_stroke: ID2D1StrokeStyle,
    pub dashed_stroke: ID2D1StrokeStyle,
    pub dim_outer: ID2D1SolidColorBrush,
    pub bar_bg: ID2D1SolidColorBrush,
    pub subbar_bg: ID2D1SolidColorBrush,
    pub surface_hover: ID2D1SolidColorBrush,
    pub border_default: ID2D1SolidColorBrush,
    pub border_subtle: ID2D1SolidColorBrush,
    pub accent_base: ID2D1SolidColorBrush,
    pub accent_hover: ID2D1SolidColorBrush,
    pub accent_press: ID2D1SolidColorBrush,
    pub accent_tint: ID2D1SolidColorBrush,
    pub accent_border: ID2D1SolidColorBrush,
    pub caution: ID2D1SolidColorBrush,
    pub text_primary: ID2D1SolidColorBrush,
    pub text_secondary: ID2D1SolidColorBrush,
    pub text_disabled: ID2D1SolidColorBrush,
    pub text_on_accent: ID2D1SolidColorBrush,
    pub spotlight_guide: ID2D1SolidColorBrush,
    pub ink_brushes: [ID2D1SolidColorBrush; 6],
    pub white_scale: f32,
    pub is_export: bool,
}

impl RenderResources {
    pub fn new(context: &ID2D1DeviceContext, white_scale: f32, is_export: bool) -> Result<Self> {
        // SAFETY: `context` is owned by the calling worker; created COM resources stay thread-local.
        unsafe {
            let factory: ID2D1Factory = context.GetFactory()?;
            let round_stroke = factory.CreateStrokeStyle(
                &D2D1_STROKE_STYLE_PROPERTIES {
                    startCap: D2D1_CAP_STYLE_ROUND,
                    endCap: D2D1_CAP_STYLE_ROUND,
                    dashCap: D2D1_CAP_STYLE_ROUND,
                    lineJoin: D2D1_LINE_JOIN_ROUND,
                    miterLimit: 4.0,
                    dashStyle: D2D1_DASH_STYLE_SOLID,
                    dashOffset: 0.0,
                },
                None,
            )?;
            let dashes = [4.0f32, 3.0f32];
            let dashed_stroke = factory.CreateStrokeStyle(
                &D2D1_STROKE_STYLE_PROPERTIES {
                    startCap: D2D1_CAP_STYLE_FLAT,
                    endCap: D2D1_CAP_STYLE_FLAT,
                    dashCap: D2D1_CAP_STYLE_FLAT,
                    lineJoin: D2D1_LINE_JOIN_MITER,
                    miterLimit: 4.0,
                    dashStyle: D2D1_DASH_STYLE_CUSTOM,
                    dashOffset: 0.0,
                },
                Some(&dashes),
            )?;
            let dim_outer = context.CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.55,
                },
                None,
            )?;
            let bar_bg = context
                .CreateSolidColorBrush(&token_rgba("surfaceWindow", white_scale, 0.96)?, None)?;
            let subbar_bg = context
                .CreateSolidColorBrush(&token_rgba("surfaceRaised", white_scale, 0.96)?, None)?;
            let surface_hover = context
                .CreateSolidColorBrush(&token_rgba("surfaceHover", white_scale, 1.0)?, None)?;
            let border_default = context
                .CreateSolidColorBrush(&token_rgba("borderDefault", white_scale, 1.0)?, None)?;
            let border_subtle = context
                .CreateSolidColorBrush(&token_rgba("borderSubtle", white_scale, 1.0)?, None)?;
            let accent_base = context
                .CreateSolidColorBrush(&token_rgba("accentBase", white_scale, 1.0)?, None)?;
            let accent_hover = context
                .CreateSolidColorBrush(&token_rgba("accentHover", white_scale, 1.0)?, None)?;
            let accent_press = context
                .CreateSolidColorBrush(&token_rgba("accentPress", white_scale, 1.0)?, None)?;
            let accent_tint = context
                .CreateSolidColorBrush(&token_rgba("accentTint", white_scale, 1.0)?, None)?;
            let accent_border = context
                .CreateSolidColorBrush(&token_rgba("accentBorder", white_scale, 1.0)?, None)?;
            let caution = context
                .CreateSolidColorBrush(&token_rgba("statusCaution", white_scale, 1.0)?, None)?;
            let text_primary = context
                .CreateSolidColorBrush(&token_rgba("textPrimary", white_scale, 1.0)?, None)?;
            let text_secondary = context
                .CreateSolidColorBrush(&token_rgba("textSecondary", white_scale, 1.0)?, None)?;
            let text_disabled = context
                .CreateSolidColorBrush(&token_rgba("textDisabled", white_scale, 1.0)?, None)?;
            let text_on_accent = context
                .CreateSolidColorBrush(&token_rgba("textOnAccent", white_scale, 1.0)?, None)?;
            let spotlight_guide = context
                .CreateSolidColorBrush(&token_rgba("textPrimary", white_scale, 0.24)?, None)?;

            let ink_brushes = [
                context.CreateSolidColorBrush(
                    &InkColor::ALL[0].d2d_color(white_scale, is_export),
                    None,
                )?,
                context.CreateSolidColorBrush(
                    &InkColor::ALL[1].d2d_color(white_scale, is_export),
                    None,
                )?,
                context.CreateSolidColorBrush(
                    &InkColor::ALL[2].d2d_color(white_scale, is_export),
                    None,
                )?,
                context.CreateSolidColorBrush(
                    &InkColor::ALL[3].d2d_color(white_scale, is_export),
                    None,
                )?,
                context.CreateSolidColorBrush(
                    &InkColor::ALL[4].d2d_color(white_scale, is_export),
                    None,
                )?,
                context.CreateSolidColorBrush(
                    &InkColor::ALL[5].d2d_color(white_scale, is_export),
                    None,
                )?,
            ];

            Ok(Self {
                round_stroke,
                dashed_stroke,
                dim_outer,
                bar_bg,
                subbar_bg,
                surface_hover,
                border_default,
                border_subtle,
                accent_base,
                accent_hover,
                accent_press,
                accent_tint,
                accent_border,
                caution,
                text_primary,
                text_secondary,
                text_disabled,
                text_on_accent,
                spotlight_guide,
                ink_brushes,
                white_scale,
                is_export,
            })
        }
    }

    pub fn update_white_scale(&mut self, white_scale: f32) -> Result<()> {
        if (self.white_scale - white_scale).abs() < 1e-6 {
            return Ok(());
        }
        // SAFETY: All brushes are owned exclusively on this worker thread.
        unsafe {
            for (brush, token, alpha) in [
                (&self.bar_bg, "surfaceWindow", 0.96),
                (&self.subbar_bg, "surfaceRaised", 0.96),
                (&self.surface_hover, "surfaceHover", 1.0),
                (&self.border_default, "borderDefault", 1.0),
                (&self.border_subtle, "borderSubtle", 1.0),
                (&self.accent_base, "accentBase", 1.0),
                (&self.accent_hover, "accentHover", 1.0),
                (&self.accent_press, "accentPress", 1.0),
                (&self.accent_tint, "accentTint", 1.0),
                (&self.accent_border, "accentBorder", 1.0),
                (&self.caution, "statusCaution", 1.0),
                (&self.text_primary, "textPrimary", 1.0),
                (&self.text_secondary, "textSecondary", 1.0),
                (&self.text_disabled, "textDisabled", 1.0),
                (&self.text_on_accent, "textOnAccent", 1.0),
                (&self.spotlight_guide, "textPrimary", 0.24),
            ] {
                let c = token_rgba(token, white_scale, alpha)?;
                brush.SetColor(&c);
            }
            for (brush, ink) in self.ink_brushes.iter().zip(InkColor::ALL) {
                let c = ink.d2d_color(white_scale, self.is_export);
                brush.SetColor(&c);
            }
        }
        self.white_scale = white_scale;
        Ok(())
    }

    pub fn ink_brush(&self, color: InkColor) -> &ID2D1SolidColorBrush {
        let idx = match color {
            InkColor::Coral => 0,
            InkColor::Amber => 1,
            InkColor::Mint => 2,
            InkColor::Cerulean => 3,
            InkColor::Chalk => 4,
            InkColor::Charcoal => 5,
        };
        &self.ink_brushes[idx]
    }
}
