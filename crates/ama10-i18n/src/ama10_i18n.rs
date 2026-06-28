//! ZetaCode IDE 国际化（i18n）基础设施。
//!
//! 提供 `tr!` 和 `tr_f!` 宏用于在 UI 中翻译字符串。
//! 通过 `init()` 初始化后，所有翻译查找基于当前语言设置进行。
//!
//! # 使用
//!
//! ```ignore
//! // 简单翻译
//! let label = tr!("No problems in workspace");
//!
//! // 带格式参数的翻译（使用 {} 占位符）
//! let msg = tr_f!("Show {count} warning", count = warning_count);
//! ```
//!
//! 翻译数据存储在 `assets/` 目录下的 TOML 文件中，
//! 以英文原文为键，目标语言翻译为值。

use gpui_shared_string::SharedString;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

/// 支持的语言/区域设置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// 英语（默认，也是源代码中的字符串）
    English,
    /// 简体中文
    Chinese,
}

/// i18n 系统的内部状态。
struct I18nInner {
    language: Language,
    /// 从英文键到翻译后字符串的映射。
    /// 对于英语，此映射为空（键即值）。
    translations: HashMap<&'static str, &'static str>,
}

/// 全局 i18n 状态。
static I18N: OnceLock<I18nInner> = OnceLock::new();

/// 初始化 i18n 系统。
///
/// 应在应用启动时调用一次。如果多次调用，后续调用将被忽略。
pub fn init(language: Language) {
    let translations = match language {
        Language::English => HashMap::new(),
        Language::Chinese => load_translations(include_str!("../assets/zh_CN.toml")),
    };
    let _ = I18N.set(I18nInner {
        language,
        translations,
    });
}

/// 从嵌入式 TOML 字符串加载翻译。
fn load_translations(toml_str: &'static str) -> HashMap<&'static str, &'static str> {
    #[derive(Deserialize)]
    struct RawTranslations {
        #[serde(flatten)]
        map: HashMap<String, String>,
    }

    let raw: RawTranslations =
        toml_edit::de::from_str(toml_str).expect("failed to parse embedded translations TOML");

    raw.map
        .into_iter()
        .map(|(key, value)| {
            let key: &'static str = Box::leak(key.into_boxed_str());
            let value: &'static str = Box::leak(value.into_boxed_str());
            (key, value)
        })
        .collect()
}

/// 翻译一个字符串键。
///
/// 在初始化后的 i18n 系统中查找 `key` 的翻译。
/// 如果未找到翻译或系统未初始化，则返回 `key` 本身（英文回退）。
pub fn translate(key: &str) -> SharedString {
    if let Some(i18n) = I18N.get() {
        if let Some(translated) = i18n.translations.get(key) {
            return SharedString::from(*translated);
        }
    }
    SharedString::from(key)
}

/// 翻译一个带 `{}` 占位符的格式化字符串。
///
/// 首先翻译 `key`，然后依次用 `args` 中的值替换 `{}` 占位符。
/// 如果翻译后的字符串中没有足够的 `{}`，多余的参数将被忽略。
pub fn translate_fmt(key: &str, args: &[&dyn std::fmt::Display]) -> SharedString {
    let template = translate(key);
    let template_str = template.as_ref();

    let mut result = String::with_capacity(template_str.len());
    let mut rest = template_str;
    for arg in args {
        if let Some(pos) = rest.find("{}") {
            result.push_str(&rest[..pos]);
            result.push_str(&format!("{}", arg));
            rest = &rest[pos + 2..];
        } else {
            result.push_str(rest);
            rest = "";
            break;
        }
    }
    result.push_str(rest);
    SharedString::from(result)
}

/// 返回当前语言设置。
pub fn current_language() -> Language {
    I18N.get()
        .map(|i18n| i18n.language)
        .unwrap_or(Language::English)
}

/// 检查 i18n 系统是否已初始化。
pub fn is_initialized() -> bool {
    I18N.get().is_some()
}

/// 简单字符串翻译宏。
///
/// 将字符串字面量翻译为当前语言的对应文本。
/// 如果未找到翻译，返回原文（英文回退）。
///
/// # 示例
///
/// ```ignore
/// let label = tr!("Save");
/// let title = tr!("Project Diagnostics");
/// ```
#[macro_export]
macro_rules! tr {
    ($key:literal) => {
        $crate::translate($key)
    };
}

/// 带格式参数的翻译宏。
///
/// 与 `tr!` 类似，但支持 `{}` 占位符，依次用提供的参数替换。
///
/// # 示例
///
/// ```ignore
/// let msg = tr_f!("Show {count} warning", count = warning_count);
/// let msg = tr_f!("Unable to save file: {err}", err = error_message);
/// ```
#[macro_export]
macro_rules! tr_f {
    ($key:literal $(, $arg:expr)* $(,)?) => {
        {
            let _args: &[&dyn std::fmt::Display] = &[$( &$arg as &dyn std::fmt::Display ),*];
            $crate::translate_fmt($key, _args)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: ensure tests work regardless of init order.
    /// Since OnceLock can only be set once, all tests share the same init.
    fn ensure_init() {
        init(Language::Chinese);
    }

    #[test]
    fn test_translate_lookup_returns_translation() {
        ensure_init();
        assert_eq!(translate("Cancel"), "取消");
        assert_eq!(translate("Save"), "保存");
    }

    #[test]
    fn test_translate_unknown_key_falls_back() {
        ensure_init();
        assert_eq!(translate("Nonexistent key"), "Nonexistent key");
    }

    #[test]
    fn test_translate_fmt_no_args() {
        ensure_init();
        assert_eq!(translate_fmt("Save", &[]), "保存");
    }

    #[test]
    fn test_translate_fmt_with_args() {
        ensure_init();
        let warning_count = 3usize;
        assert_eq!(
            translate_fmt("Show {} warnings", &[&warning_count]),
            "显示 3 个警告"
        );
    }

    #[test]
    fn test_tr_macro() {
        ensure_init();
        assert_eq!(tr!("Cancel"), "取消");
        assert_eq!(tr!("Save"), "保存");
    }

    #[test]
    fn test_tr_f_macro() {
        ensure_init();
        let count = 3;
        let count = 3;
        assert_eq!(tr_f!("Show {} warnings", count), "显示 3 个警告");
    }

    #[test]
    fn test_current_language() {
        ensure_init();
        assert_eq!(current_language(), Language::Chinese);
    }

    #[test]
    fn test_is_initialized() {
        ensure_init();
        assert!(is_initialized());
    }
}
