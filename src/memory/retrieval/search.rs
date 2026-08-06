use super::super::model::{
    decayed_strength, is_forgotten, ranking_score, MemoryHit, MemoryItem, MemoryKind, MemoryScope,
};
use anyhow::{Context, Result};
use rusqlite::{params, Connection};

/// 单次检索从数据库取回的候选上限。
///
/// 取回后还要按作用域过滤、按强度衰减重排，因此这里取得比最终注入数多。
const SEARCH_FETCH_LIMIT: usize = 40;

/// 检索与当前输入相关的记忆。
///
/// 与旧实现的两处关键差别：
/// 1. 按当前工作区过滤作用域，其它项目的记忆不会污染上下文
/// 2. 命中不再调用 reinforce——旧实现每次召回都提升强度，形成正反馈，
///    高频关键词的记忆越滚越强，长尾记忆持续衰减直到消失
///
/// 参数:
/// - `conn`: 记忆数据库连接
/// - `query`: 当前用户输入
/// - `workspace`: 当前工作区路径；无工作区时为 None
/// - `now_days`: 当前时间的天数刻度，用于计算衰减
/// - `half_life_days`: 记忆强度半衰期
///
/// 返回:
/// - 按综合得分降序的命中
pub fn search(
    conn: &Connection,
    query: &str,
    workspace: Option<&str>,
    now_days: f64,
    half_life_days: f64,
) -> Result<Vec<MemoryHit>> {
    // 两路检索各取所长：unicode61 命中西文词，trigram 命中中文子串
    let mut best: std::collections::HashMap<i64, (MemoryItem, f64)> =
        std::collections::HashMap::new();
    for (table, terms) in [
        ("memories_fts", word_terms(query)),
        ("memories_fts_tri", trigram_terms(query)),
    ] {
        if terms.is_empty() {
            continue;
        }
        let rows = query_index(conn, table, &terms)?;
        // bm25 绝对量级随语料规模变化，按本批最优值归一化再合并两路结果
        let best_rank = rows
            .iter()
            .map(|(_, rank)| *rank)
            .fold(f64::INFINITY, f64::min);
        for (item, rank) in rows {
            let relevance = relevance_from_bm25(rank, best_rank);
            best.entry(item.id)
                .and_modify(|entry| {
                    if relevance > entry.1 {
                        entry.1 = relevance;
                    }
                })
                .or_insert((item, relevance));
        }
    }

    let mut hits = Vec::new();
    for (item, relevance) in best.into_values() {
        // 1. 作用域不匹配的记忆直接丢弃，不参与排序
        if !item.scope.visible_in(workspace) {
            continue;
        }
        // 2. 按更新时间衰减强度；召回本身不改变强度
        let age_days = (now_days - days_since_epoch(&item.updated_at)).max(0.0);
        let strength = decayed_strength(item.salience, age_days, half_life_days);
        if is_forgotten(strength) {
            continue;
        }
        let score = ranking_score(relevance, strength);
        hits.push(MemoryHit {
            item,
            relevance,
            strength,
            score,
        });
    }
    hits.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            // 得分相同时按主键排序，保证结果稳定可测
            .then(left.item.id.cmp(&right.item.id))
    });
    Ok(hits)
}

/// 在指定全文索引上执行一次查询。
///
/// 参数:
/// - `conn`: 记忆数据库连接
/// - `table`: 全文索引表名
/// - `terms`: FTS5 查询串
///
/// 返回:
/// - 命中的记忆与其 bm25 排名
fn query_index(conn: &Connection, table: &str, terms: &str) -> Result<Vec<(MemoryItem, f64)>> {
    let sql = format!(
        "SELECT m.id, m.kind, m.scope_path, m.content, m.salience, m.tags,
                m.created_at, m.updated_at, bm25({table}) AS rank
         FROM {table}
         JOIN memories m ON m.id = {table}.rowid
         WHERE {table} MATCH ?1
         ORDER BY rank
         LIMIT ?2"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![terms, SEARCH_FETCH_LIMIT as i64], |row| {
        let kind_text: String = row.get(1)?;
        let scope_path: Option<String> = row.get(2)?;
        let tags_text: String = row.get(5)?;
        let rank: f64 = row.get(8)?;
        Ok((
            MemoryItem {
                id: row.get(0)?,
                kind: MemoryKind::parse(&kind_text).unwrap_or(MemoryKind::Fact),
                scope: MemoryScope::from_stored(scope_path.as_deref()),
                content: row.get(3)?,
                salience: row.get(4)?,
                tags: tags_text
                    .split(',')
                    .map(str::trim)
                    .filter(|tag| !tag.is_empty())
                    .map(str::to_string)
                    .collect(),
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            },
            rank,
        ))
    })?;
    rows.collect::<Result<Vec<_>, _>>().context("检索记忆失败")
}

/// 把 bm25 排名换算为 0 到 1 的相关度。
///
/// SQLite 的 bm25 返回负值，越小越相关，但其绝对量级取决于语料规模：
/// 索引里只有少量文档时 IDF 退化，bm25 会贴近 0，直接线性映射会把所有
/// 命中都压成接近 0 的相关度。这里改为按同批结果的排名归一化，
/// 只保留相对次序，不依赖绝对量级。
///
/// 参数:
/// - `rank`: bm25 原始值
/// - `best`: 同批结果中最优（最小）的 bm25 值
///
/// 返回:
/// - 相关度，取值 0.0 到 1.0
fn relevance_from_bm25(rank: f64, best: f64) -> f64 {
    // 最优命中记满分，其余按与最优的差距递减
    let magnitude = (-rank).max(0.0);
    let best_magnitude = (-best).max(0.0);
    if best_magnitude <= f64::EPSILON {
        // 全批 bm25 都退化到 0：命中本身即视为相关
        return 1.0;
    }
    (magnitude / best_magnitude).clamp(0.0, 1.0)
}

/// 把 RFC3339 时间换算为天数刻度。
///
/// 参数:
/// - `timestamp`: RFC3339 时间文本
///
/// 返回:
/// - 距 Unix 纪元的天数；解析失败时为 0.0
fn days_since_epoch(timestamp: &str) -> f64 {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|value| value.timestamp() as f64 / 86_400.0)
        .unwrap_or(0.0)
}

/// 提取西文查询词，用于 unicode61 索引。
///
/// 参数:
/// - `query`: 用户输入
///
/// 返回:
/// - FTS5 查询串；无有效词元时为空串
fn word_terms(query: &str) -> String {
    let mut terms: Vec<String> = Vec::new();
    let mut word = String::new();
    for ch in query.chars() {
        if ch.is_ascii_alphanumeric() {
            word.push(ch.to_ascii_lowercase());
            continue;
        }
        if !word.is_empty() {
            terms.push(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() {
        terms.push(word);
    }
    // 单字符词元噪声大，命中率低，直接丢弃
    terms.retain(|term| term.chars().count() > 1);
    join_terms(terms)
}

/// 提取中文查询词，用于 trigram 索引。
///
/// trigram 分词器按三字符滑窗建索引，查询串同样需要至少三个字符才能命中。
/// 连续 CJK 按三字滑窗切分，不足三字的片段整体作为一个查询词。
///
/// 参数:
/// - `query`: 用户输入
///
/// 返回:
/// - FTS5 查询串；无有效词元时为空串
fn trigram_terms(query: &str) -> String {
    let mut terms: Vec<String> = Vec::new();
    let mut run: Vec<char> = Vec::new();
    for ch in query.chars() {
        if is_cjk(ch) {
            run.push(ch);
            continue;
        }
        flush_cjk_run(&mut run, &mut terms);
    }
    flush_cjk_run(&mut run, &mut terms);
    join_terms(terms)
}

/// 把一段连续 CJK 字符切成 trigram 查询词。
///
/// 参数:
/// - `run`: 连续 CJK 字符，处理后清空
/// - `terms`: 词元收集表
///
/// 返回:
/// - 无；结果写入 `terms`
fn flush_cjk_run(run: &mut Vec<char>, terms: &mut Vec<String>) {
    if run.is_empty() {
        return;
    }
    if run.len() < 3 {
        // 不足三字无法产生 trigram，整体作为查询词交给 FTS5 处理
        terms.push(run.iter().collect());
    } else {
        for window in run.windows(3) {
            terms.push(window.iter().collect());
        }
    }
    run.clear();
}

/// 把词元表拼接为 FTS5 的 OR 查询串。
///
/// 参数:
/// - `terms`: 词元表，可含重复
///
/// 返回:
/// - FTS5 查询串；无词元时为空串
fn join_terms(mut terms: Vec<String>) -> String {
    terms.sort();
    terms.dedup();
    terms
        .into_iter()
        .map(|term| format!("\"{}\"", term.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" OR ")
}

/// 判断字符是否属于 CJK 表意文字。
///
/// 参数:
/// - `ch`: 待判定字符
///
/// 返回:
/// - 属于 CJK 时为 true
fn is_cjk(ch: char) -> bool {
    matches!(ch, '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::model::MemoryCandidate;
    use crate::memory::persistence::{repository, schema::ensure_schema};

    /// 2026-08-04 对应的天数刻度。
    const TODAY: f64 = 20_669.0;
    const NOW: &str = "2026-08-04T00:00:00Z";

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        ensure_schema(&conn).unwrap();
        conn
    }

    fn write(
        conn: &Connection,
        kind: MemoryKind,
        scope: MemoryScope,
        content: &str,
        at: &str,
    ) -> i64 {
        repository::insert(
            conn,
            &MemoryCandidate {
                kind,
                scope,
                content: content.to_string(),
                salience: 0.9,
                tags: Vec::new(),
            },
            at,
        )
        .unwrap()
    }

    /// 验证可以按中文内容召回。
    #[test]
    fn recalls_chinese_content() {
        let conn = conn();
        write(
            &conn,
            MemoryKind::Preference,
            MemoryScope::Global,
            "用户偏好紧凑的界面布局",
            NOW,
        );

        let hits = search(&conn, "界面布局怎么调", None, TODAY, 90.0).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].score > 0.0);
    }

    /// 验证可以按英文关键词召回。
    #[test]
    fn recalls_ascii_keywords() {
        let conn = conn();
        write(
            &conn,
            MemoryKind::Preference,
            MemoryScope::Global,
            "用户一律使用 pnpm",
            NOW,
        );

        let hits = search(&conn, "该用 pnpm 还是 npm", None, TODAY, 90.0).unwrap();
        assert_eq!(hits.len(), 1);
        // 记录实际得分量级，注入门槛必须与之匹配
        assert!(
            hits[0].score > 0.3,
            "关键词精确命中的得分过低: relevance={} strength={} score={}",
            hits[0].relevance,
            hits[0].strength,
            hits[0].score
        );
    }

    /// 验证其它工作区的项目记忆不会被召回。
    ///
    /// 这是旧实现没有的隔离：全部记忆平铺在一个池子里互相污染。
    #[test]
    fn project_memories_do_not_leak_across_workspaces() {
        let conn = conn();
        write(
            &conn,
            MemoryKind::Decision,
            MemoryScope::from_stored(Some("/home/a")),
            "构建改用 vite",
            NOW,
        );

        assert!(search(&conn, "构建改用什么", Some("/home/b"), TODAY, 90.0)
            .unwrap()
            .is_empty());
        assert_eq!(
            search(&conn, "构建改用什么", Some("/home/a"), TODAY, 90.0)
                .unwrap()
                .len(),
            1
        );
    }

    /// 验证全局记忆在任何工作区都能召回。
    #[test]
    fn global_memories_are_recalled_everywhere() {
        let conn = conn();
        write(
            &conn,
            MemoryKind::Preference,
            MemoryScope::Global,
            "用户一律使用 pnpm",
            NOW,
        );

        assert_eq!(
            search(&conn, "pnpm", Some("/home/b"), TODAY, 90.0)
                .unwrap()
                .len(),
            1
        );
    }

    /// 验证已衰减到遗忘阈值以下的记忆不再召回。
    #[test]
    fn forgotten_memories_are_not_recalled() {
        let conn = conn();
        write(
            &conn,
            MemoryKind::Episode,
            MemoryScope::Global,
            "调整了界面布局",
            "2020-01-01T00:00:00Z",
        );

        assert!(search(&conn, "界面布局", None, TODAY, 30.0)
            .unwrap()
            .is_empty());
    }

    /// 验证召回结果按综合得分降序排列。
    #[test]
    fn results_are_ordered_by_score() {
        let conn = conn();
        write(
            &conn,
            MemoryKind::Preference,
            MemoryScope::Global,
            "用户偏好 pnpm 管理依赖",
            NOW,
        );
        write(
            &conn,
            MemoryKind::Episode,
            MemoryScope::Global,
            "曾经提到过 pnpm",
            "2026-01-01T00:00:00Z",
        );

        let hits = search(&conn, "pnpm", None, TODAY, 90.0).unwrap();
        assert_eq!(hits.len(), 2);
        assert!(hits[0].score >= hits[1].score);
    }

    /// 验证召回不改变记忆强度。
    ///
    /// 旧实现每次命中都 reinforce，形成正反馈让长尾记忆饿死。
    #[test]
    fn recall_does_not_reinforce_the_memory() {
        let conn = conn();
        let id = write(
            &conn,
            MemoryKind::Preference,
            MemoryScope::Global,
            "用户一律使用 pnpm",
            NOW,
        );

        for _ in 0..5 {
            search(&conn, "pnpm", None, TODAY, 90.0).unwrap();
        }

        let item = repository::find(&conn, id).unwrap().unwrap();
        assert_eq!(item.salience, 0.9, "召回不应改变显著性");
        assert_eq!(item.updated_at, NOW, "召回不应改变更新时间");
    }

    /// 验证空查询不产生检索。
    #[test]
    fn a_blank_query_returns_nothing() {
        let conn = conn();
        write(
            &conn,
            MemoryKind::Fact,
            MemoryScope::Global,
            "用户使用 Arch Linux",
            NOW,
        );

        assert!(search(&conn, "   ", None, TODAY, 90.0).unwrap().is_empty());
    }

    /// 验证 bm25 换算保持单调性。
    #[test]
    fn bm25_conversion_is_monotonic() {
        assert!(relevance_from_bm25(-10.0, -10.0) > relevance_from_bm25(-1.0, -10.0));
        assert!(relevance_from_bm25(1.0, -10.0) >= 0.0);
        assert!(relevance_from_bm25(-1_000.0, -10.0) <= 1.0);
    }

    /// 验证 bm25 退化到 0 时仍判定为相关。
    ///
    /// 索引里文档很少时 IDF 退化，bm25 会贴近 0，
    /// 按绝对量级映射会把有效命中压成接近 0 的相关度。
    #[test]
    fn degenerate_bm25_still_counts_as_relevant() {
        assert_eq!(relevance_from_bm25(-1e-9, -1e-9), 1.0);
        assert_eq!(relevance_from_bm25(0.0, 0.0), 1.0);
    }
}
