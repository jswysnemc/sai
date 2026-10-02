use super::prompt_service::{PromptDocument, PromptKind};
use anyhow::{bail, Result};

/// 返回指定输入场景的预置模板；参数为场景，返回关键词和正文集合。
pub(super) fn presets(kind: PromptKind) -> Vec<PromptDocument> {
    let entries: &[(&str, &str)] = match kind {
        PromptKind::ChatTemplate => &[
            ("tpl-review", "请审阅以下代码，重点检查正确性、边界条件、安全性和性能。按严重程度列出问题，说明原因及修复建议。\n\n待审阅代码：\n"),
            ("tpl-debug", "请分析以下问题。先根据现象和证据定位原因，再给出最小修复方案和验证步骤。缺少信息时请明确指出。\n\n问题描述：\n"),
            ("tpl-explain", "请解释以下代码的用途、输入输出和执行流程，说明关键设计与边界条件。\n\n代码或文件：\n"),
            ("tpl-test", "请为以下功能设计测试，覆盖正常流程、边界条件和错误处理，优先验证用户可观察的行为。\n\n功能描述：\n"),
        ],
        PromptKind::ImageTemplate => &[
            ("tpl-photo", "生成一张写实摄影作品。\n主体：[描述主体]\n场景：[环境与背景]\n构图：[景别与视角]\n光线：[光源与氛围]\n要求：自然纹理，准确透视，不添加文字或水印。"),
            ("tpl-product", "生成一张商业产品摄影图。\n产品：[产品外观与材质]\n背景：[背景色与场景]\n构图：突出产品，保持轮廓清晰，预留适量留白。\n光线：柔和棚拍光，真实阴影与反射。\n要求：不添加未经指定的文字或标志。"),
            ("tpl-illustration", "创作一幅插画。\n主题：[画面内容]\n风格：[插画风格]\n色彩：[主色与辅助色]\n构图：[主体位置与层次]\n氛围：[情绪与光线]\n要求：视觉风格统一，细节清晰。"),
            ("tpl-edit", "编辑所附图片。\n需要修改：[具体区域与修改内容]\n需要保留：[主体、构图或其他细节]\n要求：保持未指定区域不变，匹配原图光线、透视和材质，过渡自然。"),
        ],
        PromptKind::Identity => &[],
    };
    entries
        .iter()
        .map(|(name, content)| PromptDocument {
            name: (*name).into(),
            content: (*content).into(),
            builtin: true,
        })
        .collect()
}

/// 校验输入模板；参数为关键词与正文，返回校验结果。
pub(super) fn validate(name: &str, content: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        bail!("Keyword must contain 1–64 ASCII letters, numbers, hyphens or underscores");
    }
    if content.trim().is_empty() || content.len() > 64 * 1024 {
        bail!("Template content must contain 1–65536 bytes");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::prompt_service::{list, read, remove, save};
    use super::*;
    use crate::paths::SaiPaths;

    /// 验证场景隔离、持久化与无损正文；无参数，无返回值。
    #[test]
    fn template_scopes_persist_independently() {
        let directory = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(directory.path());
        save(&paths, PromptKind::ChatTemplate, None, "mine", "chat\n\n").unwrap();
        save(&paths, PromptKind::ImageTemplate, None, "mine", "image").unwrap();
        assert_eq!(
            read(&paths, PromptKind::ChatTemplate, "mine")
                .unwrap()
                .content,
            "chat\n\n"
        );
        assert_eq!(
            read(&paths, PromptKind::ImageTemplate, "mine")
                .unwrap()
                .content,
            "image"
        );
        let items = list(&paths, PromptKind::ChatTemplate).unwrap();
        assert!(items
            .iter()
            .any(|item| item.name == "mine" && item.content.as_deref() == Some("chat\n\n")));
        assert!(!items.iter().any(|item| item.name == "tpl-photo"));
        assert!(read(&paths, PromptKind::ImageTemplate, "tpl-review").is_err());
    }

    /// 验证重命名冲突和预置保护；无参数，无返回值。
    #[test]
    fn templates_reject_collisions_and_protect_presets() {
        let directory = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(directory.path());
        let kind = PromptKind::ChatTemplate;
        save(&paths, kind, None, "first", "original").unwrap();
        save(&paths, kind, None, "second", "keep").unwrap();
        assert!(save(&paths, kind, Some("first"), "second", "overwrite").is_err());
        assert!(save(&paths, kind, None, "first", "overwrite").is_err());
        assert!(save(&paths, kind, None, "tpl-review", "overwrite").is_err());
        assert!(save(&paths, kind, Some("tpl-review"), "copy", "overwrite").is_err());
        assert!(remove(&paths, kind, "tpl-review").is_err());
        assert!(save(&paths, kind, Some("missing"), "new", "value").is_err());
        save(&paths, kind, Some("first"), "renamed", "changed").unwrap();
        assert!(read(&paths, kind, "first").is_err());
        assert_eq!(read(&paths, kind, "second").unwrap().content, "keep");
        assert!(remove(&paths, kind, "renamed").unwrap());
    }

    /// 验证关键词与内容边界；无参数，无返回值。
    #[test]
    fn template_input_validation() {
        for name in ["", "../escape", "a/b", "with space", "a.md", "/test"] {
            assert!(validate(name, "text").is_err());
        }
        assert!(validate("valid-name_1", "text").is_ok());
        assert!(validate("valid", "   ").is_err());
        assert!(validate("valid", &"x".repeat(65537)).is_err());
        for kind in [PromptKind::ChatTemplate, PromptKind::ImageTemplate] {
            for item in presets(kind) {
                validate(&item.name, &item.content).unwrap();
            }
        }
    }
}
