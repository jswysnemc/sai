use super::*;

/// 【游戏调查测试】【输出契约】调查结果仍要求使用最终报告及操作建议。
#[test]
fn output_instruction_mentions_final_report() {
    assert!(OUTPUT_INSTRUCTION.contains("final_report"));
    assert!(OUTPUT_INSTRUCTION.contains("红绿灯"));
    assert!(OUTPUT_INSTRUCTION.contains("怎么"));
}

/// 【游戏调查测试】【报告正文】去除引导段落并保留原有报告章节。
#[test]
fn final_report_preserves_the_investigation_sections() {
    assert_eq!(
        strip_report_preamble("以下是最终报告\n\n## 调查结果\n可玩\n\n## 怎么玩\n使用 Proton"),
        "## 调查结果\n可玩\n\n## 怎么玩\n使用 Proton"
    );
}

/// 【游戏调查测试】【依赖禁用】没有采集插件时直接报错，不初始化模型或保留隐藏采集器。
#[tokio::test]
async fn unavailable_signal_plugin_stops_before_model_initialization() {
    let root = tempfile::tempdir().unwrap();
    let context = GameCompatibilityContext {
        config: AppConfig::default(),
        paths: SaiPaths::for_tests(root.path()),
        tools: ToolRegistry::new(),
    };
    let error = linux_game_compatibility(
        json!({"game":"Cyberpunk 2077"}),
        context,
        ToolProgress::default(),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("linux-game-signals plugin"));
}
