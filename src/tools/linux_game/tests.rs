#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugifies_game_names() {
        assert_eq!(slugify("Cyberpunk 2077"), "cyberpunk-2077");
        assert_eq!(
            slugify("Tom Clancy's Rainbow Six® Siege"),
            "tom-clancy-s-rainbow-six-siege"
        );
    }

    #[test]
    fn normalizes_chinese_cyberpunk_query() {
        assert_eq!(normalize_game_query("赛博朋克2077"), "Cyberpunk 2077");
        assert_eq!(
            normalize_game_query("Linux能玩赛博朋克2077吗"),
            "Cyberpunk 2077"
        );
    }

    #[test]
    fn normalizes_chinese_genshin_query() {
        assert_eq!(normalize_game_query("原神"), "Genshin Impact");
        assert!(game_candidates("linux能玩原神吗")
            .iter()
            .any(|candidate| candidate == "Genshin Impact"));
        assert_eq!(slugify("Genshin Impact"), "genshin-impact");
        assert_eq!(
            slug_candidates(&game_candidates("linux能玩原神吗")),
            vec!["genshin-impact"]
        );
    }

    #[test]
    fn output_instruction_mentions_final_report() {
        assert!(OUTPUT_INSTRUCTION.contains("final_report"));
        assert!(OUTPUT_INSTRUCTION.contains("红绿灯"));
        assert!(OUTPUT_INSTRUCTION.contains("怎么"));
    }

    #[test]
    fn insufficient_data_requires_followup() {
        let result = verdict(&None, None, None, "");
        assert_eq!(result["label"], "不一定能玩");
        let confidence = compatibility_confidence(None, &None, None, None, &result);
        assert_eq!(confidence["level"], "low");
        assert_eq!(confidence["needs_followup"], true);
    }

    #[test]
    fn strong_cross_source_signal_is_high_confidence() {
        let protondb = Some(json!({"tier":"gold"}));
        let result = verdict(&protondb, Some("Works"), None, "");
        let confidence = compatibility_confidence(
            Some(1091500),
            &protondb,
            Some("Works"),
            Some("Running"),
            &result,
        );
        assert_eq!(result["label"], "可玩");
        assert_eq!(confidence["level"], "high");
        assert_eq!(confidence["needs_followup"], false);
    }

    #[test]
    fn genshin_can_i_play_and_anticheat_indicate_playable() {
        let result = verdict(
            &None,
            Some("Genshin Impact Works Yes — runs via Proton"),
            Some("Genshin Impact Running AntiCheat"),
            "",
        );
        assert_eq!(result["label"], "可玩");
        let confidence =
            compatibility_confidence(None, &None, Some("Works"), Some("Running"), &result);
        assert_eq!(confidence["level"], "medium");
        assert_eq!(confidence["needs_followup"], true);
    }

    #[test]
    fn single_source_signal_still_suggests_followup() {
        let protondb = Some(json!({"tier":"gold"}));
        let result = verdict(&protondb, None, None, "");
        let confidence = compatibility_confidence(Some(1091500), &protondb, None, None, &result);
        assert_eq!(confidence["level"], "medium");
        assert_eq!(confidence["needs_followup"], true);
    }

    #[test]
    fn anticheat_denied_blocks_multiplayer_verdict() {
        let result = verdict(
            &None,
            None,
            Some("Apex Legends Denied Easy Anti-Cheat"),
            "多人",
        );
        assert_eq!(result["traffic_light"], "🔴");
    }

    #[test]
    fn gold_protondb_is_playable() {
        let result = verdict(&Some(json!({"tier":"gold"})), None, None, "");
        assert_eq!(result["traffic_light"], "🟢");
    }

    #[test]
    fn can_i_play_marks_recommended_proton_as_source_value() {
        let summary = extract_can_i_play_summary(
            "<p>Works</p><p>Recommended Proton</p><p>Proton 9.0-3</p><p>Steam Deck Verified</p>",
        );
        assert_eq!(summary["source_recommended_proton"], "Proton 9.0-3");
        assert!(summary.get("recommended_proton").is_none());
    }
}
