use super::billing::billable_input_tokens;
use super::record::UsageRecord;
use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// 热力图固定 52 列，与 GitHub 贡献图同宽。
const WEEK_COUNT: i64 = 52;

/// 单日活动，空日也占位，前端按连续日历铺格子。
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct UsageDailyActivity {
    pub date: String,
    pub requests: u64,
    pub sessions: u64,
    pub tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub billable_input_tokens: u64,
}

/// 连续活跃天数。今天无活动则当前连续为 0。
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct UsageStreaks {
    pub current_streak: u64,
    pub longest_streak: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_streak_start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub longest_streak_start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub longest_streak_end: Option<String>,
}

/// 按日日历与连续活跃汇总。
#[derive(Debug, Clone, Default, Serialize)]
pub struct UsageActivity {
    pub daily_activity: Vec<UsageDailyActivity>,
    pub streaks: UsageStreaks,
    pub active_days: u64,
    pub total_days: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_activity_day: Option<String>,
}

#[derive(Default)]
struct DayAcc {
    requests: u64,
    tokens: u64,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    billable_input_tokens: u64,
    sessions: BTreeSet<String>,
}

/// 以本机今天为右端，生成 52 周热力图窗口。
///
/// 参数:
/// - `records`: 已按查询条件过滤的记录
///
/// 返回:
/// - 连续日历、连续天数与峰值日
pub(crate) fn build_activity(records: &[UsageRecord]) -> UsageActivity {
    build_activity_as_of(records, Local::now().date_naive())
}

/// 按指定结束日生成日历，测试可固定日期。
///
/// 参数:
/// - `records`: 已过滤的记录
/// - `today`: 窗口右端（含）
///
/// 返回:
/// - 从最近一个周日往前 51 整周起、到今天止的连续日
pub(crate) fn build_activity_as_of(records: &[UsageRecord], today: NaiveDate) -> UsageActivity {
    // 1. 窗口从今天所在周的周日往前 51 整周开始，空日也保留
    let week_start = start_of_week_sunday(today);
    let start = week_start - Duration::days((WEEK_COUNT - 1) * 7);
    let grouped = group_by_day(records);
    let mut daily_activity = Vec::new();
    let mut cursor = start;
    while cursor <= today {
        daily_activity.push(finish_day(cursor, grouped.get(&cursor)));
        cursor += Duration::days(1);
    }
    // 2. 连续天数与峰值只看窗口内有请求的日子
    let streaks = compute_streaks(&daily_activity);
    let active_days = daily_activity
        .iter()
        .filter(|day| day.requests > 0)
        .count() as u64;
    let peak_activity_day = daily_activity
        .iter()
        .filter(|day| day.tokens > 0)
        .max_by_key(|day| day.tokens)
        .map(|day| day.date.clone());
    UsageActivity {
        total_days: daily_activity.len() as u64,
        daily_activity,
        streaks,
        active_days,
        peak_activity_day,
    }
}

fn start_of_week_sunday(date: NaiveDate) -> NaiveDate {
    let offset = date.weekday().num_days_from_sunday() as i64;
    date - Duration::days(offset)
}

fn group_by_day(records: &[UsageRecord]) -> BTreeMap<NaiveDate, DayAcc> {
    let mut map: BTreeMap<NaiveDate, DayAcc> = BTreeMap::new();
    for record in records {
        let Some(date) = Local
            .timestamp_opt(record.created_at, 0)
            .single()
            .map(|dt| dt.date_naive())
        else {
            continue;
        };
        let acc = map.entry(date).or_default();
        acc.requests = acc.requests.saturating_add(1);
        acc.tokens = acc.tokens.saturating_add(record.total_tokens_or_sum());
        acc.input_tokens = acc
            .input_tokens
            .saturating_add(record.input_tokens.unwrap_or(0));
        acc.output_tokens = acc
            .output_tokens
            .saturating_add(record.output_tokens.unwrap_or(0));
        acc.cache_read_tokens = acc.cache_read_tokens.saturating_add(record.cache_read());
        acc.billable_input_tokens = acc
            .billable_input_tokens
            .saturating_add(billable_input_tokens(record));
        if let Some(session_id) = record
            .session_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            acc.sessions.insert(session_id.to_string());
        }
    }
    map
}

fn finish_day(date: NaiveDate, acc: Option<&DayAcc>) -> UsageDailyActivity {
    let Some(acc) = acc else {
        return UsageDailyActivity {
            date: date.to_string(),
            ..UsageDailyActivity::default()
        };
    };
    UsageDailyActivity {
        date: date.to_string(),
        requests: acc.requests,
        sessions: acc.sessions.len() as u64,
        tokens: acc.tokens,
        input_tokens: acc.input_tokens,
        output_tokens: acc.output_tokens,
        cache_read_tokens: acc.cache_read_tokens,
        billable_input_tokens: acc.billable_input_tokens,
    }
}

fn compute_streaks(days: &[UsageDailyActivity]) -> UsageStreaks {
    let mut longest_streak = 0u64;
    let mut longest_start: Option<String> = None;
    let mut longest_end: Option<String> = None;
    let mut run = 0u64;
    let mut run_start: Option<String> = None;
    for day in days {
        if day.requests > 0 {
            if run == 0 {
                run_start = Some(day.date.clone());
            }
            run = run.saturating_add(1);
            if run >= longest_streak {
                longest_streak = run;
                longest_start = run_start.clone();
                longest_end = Some(day.date.clone());
            }
        } else {
            run = 0;
            run_start = None;
        }
    }
    let mut current_streak = 0u64;
    let mut current_start: Option<String> = None;
    for day in days.iter().rev() {
        if day.requests == 0 {
            break;
        }
        current_streak = current_streak.saturating_add(1);
        current_start = Some(day.date.clone());
    }
    UsageStreaks {
        current_streak,
        longest_streak,
        current_streak_start: current_start,
        longest_streak_start: longest_start,
        longest_streak_end: longest_end,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage_history::record::UsageRecord;
    use chrono::{NaiveDate, Weekday};

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    fn ts(day: NaiveDate) -> i64 {
        Local
            .from_local_datetime(&day.and_hms_opt(12, 0, 0).unwrap())
            .single()
            .unwrap()
            .timestamp()
    }

    fn record(day: NaiveDate, session: Option<&str>, tokens: u64) -> UsageRecord {
        UsageRecord {
            id: format!("{}-{}", session.unwrap_or("none"), day),
            created_at: ts(day),
            completed_at: ts(day),
            duration_ms: 10,
            source: "chat".to_string(),
            operation: "turn".to_string(),
            provider_id: "p1".to_string(),
            provider_name: "Provider".to_string(),
            model: "model-a".to_string(),
            status: "success".to_string(),
            usage_source: "provider_reported".to_string(),
            input_tokens: Some(tokens),
            output_tokens: Some(0),
            total_tokens: Some(tokens),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
            session_id: session.map(str::to_string),
            workspace_id: None,
            error_kind: None,
        }
    }

    /// 验证 52 周窗口含空日占位，且不包含明天。
    #[test]
    fn pads_empty_days_and_stops_at_today() {
        let today = date(2026, 5, 9);
        let activity = build_activity_as_of(&[record(today, Some("a"), 128_000)], today);
        assert_eq!(activity.daily_activity.last().unwrap().date, "2026-05-09");
        assert!(activity
            .daily_activity
            .iter()
            .all(|day| day.date.as_str() <= "2026-05-09"));
        assert!(activity.total_days >= 358);
        assert!(activity.total_days <= 364);
        let empty = activity
            .daily_activity
            .iter()
            .filter(|day| day.requests == 0)
            .count();
        assert!(empty > 300);
        let today_row = activity.daily_activity.last().unwrap();
        assert_eq!(today_row.requests, 1);
        assert_eq!(today_row.sessions, 1);
        assert_eq!(today_row.tokens, 128_000);
        assert_eq!(activity.active_days, 1);
        assert_eq!(activity.peak_activity_day.as_deref(), Some("2026-05-09"));
    }

    /// 验证同一天同一会话只计一次会话，请求仍累加。
    #[test]
    fn counts_unique_sessions_per_day() {
        let today = date(2026, 5, 9);
        let records = vec![
            record(today, Some("a"), 10),
            record(today, Some("a"), 20),
            record(today, Some("b"), 5),
            record(today, None, 100),
        ];
        let activity = build_activity_as_of(&records, today);
        let today_row = activity.daily_activity.last().unwrap();
        assert_eq!(today_row.requests, 4);
        assert_eq!(today_row.sessions, 2);
        assert_eq!(today_row.tokens, 135);
    }

    /// 验证当前连续在今天中断时归零，最长连续保留历史区间。
    #[test]
    fn computes_current_and_longest_streaks() {
        let today = date(2026, 5, 9);
        let records = vec![
            record(date(2026, 3, 1), Some("a"), 1),
            record(date(2026, 3, 2), Some("a"), 1),
            record(date(2026, 3, 3), Some("a"), 1),
            record(date(2026, 5, 7), Some("a"), 1),
            record(date(2026, 5, 8), Some("a"), 1),
            record(date(2026, 5, 9), Some("a"), 1),
        ];
        let activity = build_activity_as_of(&records, today);
        assert_eq!(activity.streaks.current_streak, 3);
        assert_eq!(
            activity.streaks.current_streak_start.as_deref(),
            Some("2026-05-07")
        );
        assert_eq!(activity.streaks.longest_streak, 3);
        let idle_today = build_activity_as_of(&[record(date(2026, 5, 7), Some("a"), 1)], today);
        assert_eq!(idle_today.streaks.current_streak, 0);
        assert_eq!(idle_today.streaks.longest_streak, 1);
    }

    /// 验证窗口起点落在周日，格子可以按周列铺开。
    #[test]
    fn window_starts_on_sunday() {
        let today = date(2026, 5, 9);
        let activity = build_activity_as_of(&[], today);
        let start = NaiveDate::parse_from_str(&activity.daily_activity[0].date, "%Y-%m-%d").unwrap();
        assert_eq!(start.weekday(), Weekday::Sun);
    }
}
