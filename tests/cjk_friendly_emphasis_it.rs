use marco_core::{parse, render, RenderOptions};

fn html(input: &str) -> String {
    let doc = parse(input).expect("parse failed");
    render(&doc, &RenderOptions::default()).expect("render failed")
}

#[test]
fn cjk_bold_closes_after_cjk_punctuation_before_hangul() {
    assert_eq!(
        html("**강조。**다음"),
        "<p><strong>강조。</strong>다음</p>\n"
    );
}

#[test]
fn cjk_bold_closes_after_ideographic_full_stop_before_han() {
    assert_eq!(
        html("**单算 Runtime 执行引擎，平均每题 29.78 秒。**边界是开始执行到产出结果。"),
        "<p><strong>单算 Runtime 执行引擎，平均每题 29.78 秒。</strong>边界是开始执行到产出结果。</p>\n"
    );
}

#[test]
fn cjk_bold_closes_after_corner_bracket() {
    assert_eq!(
        html("**「인용」**다음"),
        "<p><strong>「인용」</strong>다음</p>\n"
    );
}

#[test]
fn cjk_emphasis_closes_after_cjk_punctuation() {
    assert_eq!(html("*강조。*다음"), "<p><em>강조。</em>다음</p>\n");
}

#[test]
fn cjk_bold_opens_after_han_before_cjk_punctuation() {
    assert_eq!(
        html("漢字**。**漢字"),
        "<p>漢字<strong>。</strong>漢字</p>\n"
    );
}

#[test]
fn cjk_rules_keep_commonmark_flanking_between_latin_letters() {
    assert_eq!(html("**Note.**Next"), "<p>**Note.**Next</p>\n");
}

#[test]
fn cjk_rules_keep_intraword_strong() {
    assert_eq!(html("a**b**c"), "<p>a<strong>b</strong>c</p>\n");
}

#[test]
fn cjk_rules_keep_intraword_underscore_literal() {
    assert_eq!(html("漢字_x_漢字"), "<p>漢字_x_漢字</p>\n");
}

#[test]
fn cjk_variation_selector_takes_the_class_of_the_character_before_it() {
    assert_eq!(
        html("**Note.\u{FE0E}**Next"),
        "<p>**Note.\u{FE0E}**Next</p>\n"
    );
}

#[test]
fn cjk_quote_with_variation_selector_one_counts_as_cjk_punctuation() {
    assert_eq!(
        html("**\u{201C}quote\u{201D}\u{FE01}**next"),
        "<p><strong>\u{201C}quote\u{201D}\u{FE01}</strong>next</p>\n"
    );
}
