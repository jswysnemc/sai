use super::super::model::{MemoryHit, MemoryKind};

/// 把选中的记忆渲染为注入上下文的文本。
///
/// 与旧实现的关键差别在措辞：旧版写「可能相关也可能不相关；必要时使用，
/// 不要强行引用」，这是在教模型忽略它。这里按类型分节，用明确断言陈述，
/// 因为能走到这一步的记忆已经过了相关度门槛。
///
/// 参数:
/// - `hits`: 已通过阈值并排序的命中
///
/// 返回:
/// - 注入文本；无命中时为 None
pub fn render_injection(hits: &[MemoryHit]) -> Option<String> {
    if hits.is_empty() {
        return None;
    }
    let mut output = String::from("<memory>\n");
    output.push_str("以下是关于该用户的既有信息，来自过往对话。请在本轮中遵循这些偏好与决策。\n");
    // 按类型分节，让偏好与决策这类强约束不被往事稀释
    for kind in [
        MemoryKind::Preference,
        MemoryKind::Decision,
        MemoryKind::Fact,
        MemoryKind::Episode,
    ] {
        let section = hits
            .iter()
            .filter(|hit| hit.item.kind == kind)
            .collect::<Vec<_>>();
        if section.is_empty() {
            continue;
        }
        output.push_str(&format!("\n{}：\n", kind.section_title()));
        for hit in section {
            output.push_str(&format!("- {}\n", hit.item.content));
        }
    }
    output.push_str("</memory>");
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::model::{MemoryItem, MemoryScope};

    fn hit(kind: MemoryKind, content: &str) -> MemoryHit {
        MemoryHit {
            item: MemoryItem {
                id: 1,
                kind,
                scope: MemoryScope::Global,
                content: content.to_string(),
                salience: 1.0,
                tags: Vec::new(),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            },
            relevance: 0.9,
            strength: 1.0,
            score: 0.9,
        }
    }

    /// 验证无命中时不产生注入文本。
    #[test]
    fn renders_nothing_without_hits() {
        assert!(render_injection(&[]).is_none());
    }

    /// 验证注入文本不含自我否定的措辞。
    ///
    /// 这正是旧实现让召回失效的原因。
    #[test]
    fn the_wording_does_not_undermine_itself() {
        let rendered =
            render_injection(&[hit(MemoryKind::Preference, "用户一律使用 pnpm")]).unwrap();
        assert!(!rendered.contains("可能相关"));
        assert!(!rendered.contains("不要强行"));
        assert!(rendered.contains("请在本轮中遵循"));
    }

    /// 验证按类型分节且偏好排在往事之前。
    #[test]
    fn groups_by_kind_with_preferences_first() {
        let rendered = render_injection(&[
            hit(MemoryKind::Episode, "上次修复了构建脚本"),
            hit(MemoryKind::Preference, "用户一律使用 pnpm"),
        ])
        .unwrap();
        let preference_at = rendered.find("用户的既定偏好").unwrap();
        let episode_at = rendered.find("相关往事").unwrap();
        assert!(preference_at < episode_at);
    }

    /// 验证空小节不会产生标题。
    #[test]
    fn omits_sections_without_hits() {
        let rendered = render_injection(&[hit(MemoryKind::Fact, "用户使用 Arch Linux")]).unwrap();
        assert!(!rendered.contains("相关往事"));
        assert!(rendered.contains("已知事实"));
    }

    /// 验证每条记忆都出现在输出里。
    #[test]
    fn every_hit_appears_in_the_output() {
        let rendered = render_injection(&[
            hit(MemoryKind::Preference, "用户一律使用 pnpm"),
            hit(MemoryKind::Decision, "构建改用 vite"),
        ])
        .unwrap();
        assert!(rendered.contains("用户一律使用 pnpm"));
        assert!(rendered.contains("构建改用 vite"));
    }
}
