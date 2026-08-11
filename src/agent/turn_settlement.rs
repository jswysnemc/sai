use crate::state::PendingTurnGuard;
use anyhow::Result;

/// 把一次可能失败的准备步骤结果落到轮次终态上。
///
/// 轮次守卫在析构时只能写一句占位文案（"本轮在写入终态前结束"），
/// 它没有任何错误信息。准备阶段的每个 `?` 若直接返回，真实原因就被这句
/// 占位文案覆盖，界面上只剩一条无法排查的提示。这里在返回前先把真实错误
/// 写进轮次，让时间线保留上游报文。
///
/// 参数:
/// - `guard`: 当前轮守卫
/// - `outcome`: 准备步骤结果
///
/// 返回:
/// - 原样透传的步骤结果；失败时已写入轮次终态
pub(super) fn settle_step<T>(
    guard: &mut PendingTurnGuard,
    outcome: Result<T>,
) -> Result<T> {
    match outcome {
        Ok(value) => Ok(value),
        Err(error) => {
            // 落终态失败不覆盖原始错误：调用方要看到的是上游原因，
            // 而不是二次写库的问题
            let _ = guard.fail_in_place(&crate::llm::error_detail_text(&error));
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::SaiPaths;
    use crate::state::{PartialTurnSink, StateStore, TurnStatus};
    use std::path::PathBuf;

    /// 构造仅指定状态目录的测试路径集合。
    ///
    /// 参数:
    /// - `state_dir`: 状态目录
    ///
    /// 返回:
    /// - 测试用路径集合
    fn test_paths(state_dir: PathBuf) -> SaiPaths {
        SaiPaths {
            config_dir: PathBuf::new(),
            config_file: PathBuf::new(),
            secrets_file: PathBuf::new(),
            skills_dir: PathBuf::new(),
            data_dir: PathBuf::new(),
            cache_dir: PathBuf::new(),
            state_dir,
            pictures_dir: PathBuf::new(),
            fish_hook_file: PathBuf::new(),
            bash_hook_file: PathBuf::new(),
            zsh_hook_file: PathBuf::new(),
            powershell_hook_file: PathBuf::new(),
        }
    }

    /// 构造一个带运行中轮次的临时会话状态。
    ///
    /// 参数:
    /// - `turn_id`: 轮次标识
    ///
    /// 返回:
    /// - 临时目录句柄与状态存储
    fn running_turn(turn_id: &str) -> (tempfile::TempDir, StateStore) {
        let dir = tempfile::tempdir().unwrap();
        let state = StateStore::new(&test_paths(dir.path().to_path_buf())).unwrap();
        state.start_turn(turn_id, "hello").unwrap();
        (dir, state)
    }

    /// 准备步骤失败时轮次落为失败并保留原始错误。
    #[test]
    fn failed_step_records_the_real_error() {
        let (_dir, state) = running_turn("turn-failed");
        let mut guard = PendingTurnGuard::new(
            state.clone(),
            "turn-failed".to_string(),
            PartialTurnSink::new(),
        );

        let outcome: Result<()> = settle_step(
            &mut guard,
            Err(anyhow::anyhow!("provider refused the request")),
        );
        drop(guard);

        assert!(outcome.is_err());
        let turns = state.load_turns().unwrap();
        let turn = turns.last().expect("turn must exist");
        assert_eq!(turn.status, TurnStatus::Failed);
        // 占位文案会盖掉真实原因，这里确认落库的是上游错误本身
        assert!(turn.assistant_content.contains("provider refused"));
    }

    /// 准备步骤成功时不写终态，后续步骤仍可继续。
    #[test]
    fn successful_step_leaves_the_turn_open() {
        let (_dir, state) = running_turn("turn-open");
        let mut guard =
            PendingTurnGuard::new(state.clone(), "turn-open".to_string(), PartialTurnSink::new());

        let value = settle_step(&mut guard, Ok(7u8)).unwrap();
        guard.complete("done", None).unwrap();

        assert_eq!(value, 7);
        let turns = state.load_turns().unwrap();
        assert_eq!(
            turns.last().expect("turn must exist").status,
            TurnStatus::Completed
        );
    }
}
