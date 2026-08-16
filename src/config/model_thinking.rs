use serde_json::Value;

/// Sai 支持的全部思考等级，按强度升序。
///
/// `auto` 不在其列：它表示不发送思考参数、交给服务商决定，
/// 因此对任何模型都可用，不参与"该模型支持哪些等级"的判定。
pub const THINKING_LEVELS: [&str; 6] = ["none", "low", "medium", "high", "xhigh", "max"];

/// 表示"由服务商决定"的等级。
pub const THINKING_LEVEL_AUTO: &str = "auto";

/// 把模型目录给出的等级值映射为 Sai 等级。
///
/// models.dev 的 `none` 与 `minimal` 是两个档位，Sai 只有一个 `none`——
/// 它在 chat 协议下发 `thinking: {type: disabled}`，在 effort 协议下发
/// `reasoning_effort: minimal`，两种语义都覆盖，因此合并到同一档。
///
/// 参数:
/// - `value`: 目录给出的原始等级值
///
/// 返回:
/// - 对应的 Sai 等级；无法对应时返回 None
pub fn map_catalog_thinking_level(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" | "off" | "disabled" | "minimal" => Some("none"),
        "low" => Some("low"),
        "medium" | "mid" => Some("medium"),
        "high" => Some("high"),
        "xhigh" | "very_high" | "veryhigh" => Some("xhigh"),
        "max" | "maximum" | "ultra" => Some("max"),
        _ => None,
    }
}

/// 从模型目录的 `reasoning_options` 推导可用思考等级。
///
/// 三种形态各有含义：
/// - `effort` 带 values，逐项映射即为可用等级
/// - `toggle` 只有开关，除关闭外无法表达强度，因此只留 `none`；
///   "开"由永远可用的 `auto` 承担
/// - `budget_tokens` 用 token 预算表达强度，Sai 的每一档都能映射出
///   不同预算，因此全部可用
///
/// 参数:
/// - `options`: models.dev 的 `reasoning_options` 数组
///
/// 返回:
/// - 按强度升序排列的可用等级；无法判定时返回空表示全部可用
pub fn thinking_levels_from_reasoning_options(options: &Value) -> Vec<String> {
    let Some(items) = options.as_array() else {
        return Vec::new();
    };
    let mut levels: Vec<&'static str> = Vec::new();
    for item in items {
        match item.get("type").and_then(Value::as_str) {
            // 预算式无法枚举档位，出现即视为全部可用
            Some("budget_tokens") => return Vec::new(),
            Some("toggle") => push_level(&mut levels, "none"),
            _ => {
                let Some(values) = item.get("values").and_then(Value::as_array) else {
                    continue;
                };
                for value in values {
                    if let Some(level) = value.as_str().and_then(map_catalog_thinking_level) {
                        push_level(&mut levels, level);
                    }
                }
            }
        }
    }
    sorted_levels(levels)
}

/// 把请求的等级落到该模型实际支持的档位上。
///
/// 用户换模型时配置里留着的旧等级不会自动失效，直接发出去会被服务端拒绝。
/// 这里就近降级而不是回退到 auto：auto 交由服务商决定，可能比用户要的更弱，
/// 把"我要重思考"悄悄变成"随便"。
///
/// 参数:
/// - `available`: 可用等级集合；空表示未知，原样返回
/// - `level`: 请求的等级
///
/// 返回:
/// - 落到可用档位后的等级
pub fn resolve_thinking_level(available: &[String], level: &str) -> String {
    let level = level.trim();
    if thinking_level_available(available, level) {
        return level.to_string();
    }
    let Some(requested) = level_rank(level) else {
        return level.to_string();
    };
    let mut ranked: Vec<(usize, &String)> = available
        .iter()
        .filter_map(|item| level_rank(item).map(|rank| (rank, item)))
        .collect();
    ranked.sort_by_key(|(rank, _)| *rank);
    // 1. 优先取不超过请求强度的最强档
    if let Some((_, level)) = ranked.iter().rev().find(|(rank, _)| *rank <= requested) {
        return (*level).clone();
    }
    // 2. 可用档位都比请求更强时取其中最弱的
    ranked
        .first()
        .map(|(_, level)| (*level).clone())
        .unwrap_or_else(|| level.to_string())
}

/// 返回等级在强度序列中的位置。
///
/// 参数:
/// - `level`: 等级名称
///
/// 返回:
/// - 强度序号；非法等级返回 None
fn level_rank(level: &str) -> Option<usize> {
    THINKING_LEVELS
        .iter()
        .position(|item| *item == level.trim())
}

/// 判断某个等级对给定的可用集合是否成立。
///
/// 参数:
/// - `available`: 可用等级集合；空表示未知，一律放行
/// - `level`: 待判定的等级
///
/// 返回:
/// - 等级可用时为真
pub fn thinking_level_available(available: &[String], level: &str) -> bool {
    let level = level.trim();
    // auto 表示不发送思考参数，对任何模型都成立
    if level.is_empty() || level == THINKING_LEVEL_AUTO {
        return true;
    }
    available.is_empty() || available.iter().any(|item| item == level)
}

/// 规范化一组等级：过滤未知值、去重并按强度升序。
///
/// 参数:
/// - `levels`: 原始等级列表
///
/// 返回:
/// - 规范化后的等级列表
pub fn normalize_thinking_levels<I, S>(levels: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut collected: Vec<&'static str> = Vec::new();
    for level in levels {
        if let Some(level) = map_catalog_thinking_level(level.as_ref()) {
            push_level(&mut collected, level);
        }
    }
    sorted_levels(collected)
}

/// 追加一个尚未出现的等级。
///
/// 参数:
/// - `levels`: 已收集的等级
/// - `level`: 待追加等级
///
/// 返回:
/// - 无
fn push_level(levels: &mut Vec<&'static str>, level: &'static str) {
    if !levels.contains(&level) {
        levels.push(level);
    }
}

/// 按 THINKING_LEVELS 的顺序排序并转为字符串。
///
/// 参数:
/// - `levels`: 已去重的等级
///
/// 返回:
/// - 升序排列的等级列表
fn sorted_levels(mut levels: Vec<&'static str>) -> Vec<String> {
    levels.sort_by_key(|level| {
        THINKING_LEVELS
            .iter()
            .position(|item| item == level)
            .unwrap_or(usize::MAX)
    });
    levels.into_iter().map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 验证 effort 形态逐项映射为 Sai 等级。
    #[test]
    fn effort_values_map_to_sai_levels() {
        let options = json!([{ "type": "effort", "values": ["high", "max"] }]);

        assert_eq!(
            thinking_levels_from_reasoning_options(&options),
            vec!["high", "max"]
        );
    }

    /// 验证 none 与 minimal 合并为同一档。
    ///
    /// 两者在 Sai 里都落到 none：目录同时给出时不应产生重复项。
    #[test]
    fn none_and_minimal_collapse_into_one_level() {
        let options = json!([{
            "type": "effort",
            "values": ["none", "minimal", "low", "high"]
        }]);

        assert_eq!(
            thinking_levels_from_reasoning_options(&options),
            vec!["none", "low", "high"]
        );
    }

    /// 验证结果按强度升序，与目录里的顺序无关。
    #[test]
    fn levels_are_sorted_by_strength() {
        let options = json!([{
            "type": "effort",
            "values": ["max", "low", "xhigh", "medium"]
        }]);

        assert_eq!(
            thinking_levels_from_reasoning_options(&options),
            vec!["low", "medium", "xhigh", "max"]
        );
    }

    /// 验证开关式只留关闭档。
    ///
    /// 开关无法表达强度，把 low/high 摆出来只会让用户以为选了有用。
    #[test]
    fn toggle_only_exposes_the_off_level() {
        let options = json!([{ "type": "toggle" }]);

        assert_eq!(
            thinking_levels_from_reasoning_options(&options),
            vec!["none"]
        );
    }

    /// 验证预算式视为全部可用。
    #[test]
    fn budget_tokens_leaves_every_level_available() {
        let options = json!([{ "type": "budget_tokens", "min": 1024, "max": 32000 }]);

        assert!(thinking_levels_from_reasoning_options(&options).is_empty());
    }

    /// 验证预算式与 effort 并存时仍视为全部可用。
    #[test]
    fn budget_tokens_wins_over_a_sibling_effort_option() {
        let options = json!([
            { "type": "effort", "values": ["high"] },
            { "type": "budget_tokens", "min": 1024, "max": 32000 }
        ]);

        assert!(thinking_levels_from_reasoning_options(&options).is_empty());
    }

    /// 验证无法识别的值被丢弃而不是原样保留。
    #[test]
    fn unknown_values_are_dropped() {
        let options = json!([{ "type": "effort", "values": ["turbo", "high"] }]);

        assert_eq!(
            thinking_levels_from_reasoning_options(&options),
            vec!["high"]
        );
    }

    /// 验证空集合表示未知，任何等级都放行。
    ///
    /// 目录没覆盖到的模型不该被锁死可选项。
    #[test]
    fn empty_availability_allows_every_level() {
        assert!(thinking_level_available(&[], "xhigh"));
    }

    /// 验证非空集合外的等级被判定为不可用。
    #[test]
    fn levels_outside_the_set_are_unavailable() {
        let available = vec!["high".to_string(), "max".to_string()];

        assert!(thinking_level_available(&available, "high"));
        assert!(!thinking_level_available(&available, "low"));
    }

    /// 验证 auto 对任何可用集合都成立。
    #[test]
    fn auto_is_always_available() {
        let available = vec!["high".to_string()];

        assert!(thinking_level_available(&available, THINKING_LEVEL_AUTO));
    }

    /// 验证可用等级原样保留。
    #[test]
    fn available_level_is_kept_as_is() {
        let available = vec!["high".to_string(), "max".to_string()];

        assert_eq!(resolve_thinking_level(&available, "max"), "max");
    }

    /// 验证超出范围的等级降到不超过它的最强档。
    #[test]
    fn unavailable_level_falls_back_to_the_strongest_below_it() {
        let available = vec!["low".to_string(), "high".to_string()];

        assert_eq!(resolve_thinking_level(&available, "xhigh"), "high");
    }

    /// 验证请求低于全部可用档位时取其中最弱的。
    ///
    /// 只支持 high/max 的模型收到 low，没有更弱的档可落，
    /// 取 high 而不是把它当成不思考。
    #[test]
    fn level_below_every_option_falls_back_to_the_weakest() {
        let available = vec!["high".to_string(), "max".to_string()];

        assert_eq!(resolve_thinking_level(&available, "low"), "high");
    }

    /// 验证未知可用集合不改写请求等级。
    #[test]
    fn unknown_availability_keeps_the_requested_level() {
        assert_eq!(resolve_thinking_level(&[], "xhigh"), "xhigh");
    }

    /// 验证 auto 不被降级。
    ///
    /// auto 的语义是不发送思考参数，改写它等于替用户做了没要求的决定。
    #[test]
    fn auto_is_never_downgraded() {
        let available = vec!["high".to_string()];

        assert_eq!(
            resolve_thinking_level(&available, THINKING_LEVEL_AUTO),
            THINKING_LEVEL_AUTO
        );
    }
}
