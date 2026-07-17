#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_leading_chinese_date_and_weekday_from_title() {
        assert_eq!(
            strip_title_date_prefix("2026年6月29日周一夏季早餐推荐"),
            "夏季早餐推荐"
        );
        assert_eq!(
            strip_title_date_prefix("2026年06月29日 星期一：夏季早餐推荐"),
            "夏季早餐推荐"
        );
    }

    #[test]
    fn extracts_report_date_suffix_from_title() {
        assert_eq!(
            report_date_suffix("2026年6月29日周一夏季早餐推荐").as_deref(),
            Some("20260629")
        );
        assert_eq!(
            report_date_suffix("夏季早餐推荐 2026-06-29").as_deref(),
            Some("20260629")
        );
    }
}
