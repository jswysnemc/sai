#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_library_keeps_simple_names() {
        assert_eq!(sanitize_library("Sai"), "sai");
        assert_eq!(sanitize_library("默认 表情"), "default");
    }

    #[test]
    fn scores_tag_matches_higher_than_no_match() {
        let item = MemeItem {
            id: "sha256:test".to_string(),
            name: LocalizedName {
                zh: "Linux 企鹅".to_string(),
                en: "Linux Penguin".to_string(),
            },
            file: "images/test.png".to_string(),
            mime_type: "image/png".to_string(),
            animated: false,
            description: "戴墨镜的企鹅抱着终端".to_string(),
            usage: "适合 Linux 话题".to_string(),
            avoid: String::new(),
            tags: vec!["Linux".to_string(), "企鹅".to_string()],
        };
        assert!(score_meme(&item, "Linux", &[]) > score_meme(&item, "炸鸡", &[]));
    }

    #[test]
    fn matches_full_prefixed_and_short_ids() {
        let id = "sha256:abcdef1234567890";
        assert!(ids_match(id, "sha256:abcdef1234567890"));
        assert!(ids_match(id, "abcdef1234567890"));
        assert!(ids_match(id, "abcdef12"));
        assert!(!ids_match(id, "123456"));
    }
}
