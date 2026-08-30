use crate::error::MarcoError;
use katex::{
    render_to_string as katex_render, KatexContext, OutputFormat, Settings as KatexSettings,
};
use std::sync::OnceLock;

// Global KaTeX context (reused across renders for performance)
static KATEX_CONTEXT: OnceLock<KatexContext> = OnceLock::new();

/// Render inline math using KaTeX.
pub(crate) fn render_inline_math(latex: &str) -> Result<String, MarcoError> {
    let ctx = KATEX_CONTEXT.get_or_init(KatexContext::default);
    let settings = KatexSettings::builder()
        .output(OutputFormat::Mathml)
        .build();
    // Stringified rather than carried: `katex-rs` lives behind `render-math`,
    // and a variant holding its error type would make the public API depend on
    // that feature.
    let html = katex_render(ctx, latex, &settings).map_err(|e| MarcoError::math(e.to_string()))?;
    Ok(normalize_math_attrs(html))
}

/// Render display math using KaTeX.
pub(crate) fn render_display_math(latex: &str) -> Result<String, MarcoError> {
    let ctx = KATEX_CONTEXT.get_or_init(KatexContext::default);
    let settings = KatexSettings::builder()
        .display_mode(true)
        .output(OutputFormat::Mathml)
        .build();
    // Stringified rather than carried: `katex-rs` lives behind `render-math`,
    // and a variant holding its error type would make the public API depend on
    // that feature.
    let html = katex_render(ctx, latex, &settings).map_err(|e| MarcoError::math(e.to_string()))?;
    Ok(normalize_math_attrs(html))
}

/// Normalize the attribute order on `<math>` elements so output is deterministic
/// regardless of KaTeX's internal initialization order. Always emits
/// `xmlns` before `display`.
fn normalize_math_attrs(html: String) -> String {
    html.replace(
        r#"<math display="block" xmlns="http://www.w3.org/1998/Math/MathML">"#,
        r#"<math xmlns="http://www.w3.org/1998/Math/MathML" display="block">"#,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_test_render_inline_math() {
        let html = render_inline_math("E = mc^2").expect("inline math should render");
        assert!(html.contains("<math") || html.contains("katex"));
    }

    #[test]
    fn smoke_test_render_display_math() {
        let html = render_display_math(r"\\frac{a}{b}").expect("display math should render");
        assert!(html.contains("<math") || html.contains("katex"));
    }
}
