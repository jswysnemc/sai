#!/usr/bin/env bash
# 【CI】【远程监控】轮询 GitHub origin/main，发现新提交后获取代码并编译验证
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

INTERVAL=${1:-60}
BASELINE=$(git rev-parse origin/main) || exit 1
echo "【CI】【远程监控】启动，基线提交: $BASELINE，轮询间隔: ${INTERVAL}s"

while true; do
  # 1. 【CI】【远程监控】获取远程引用，网络失败留待下轮重试
  if ! git fetch --quiet origin main 2>>scripts/.remote-monitor.err; then
    echo "【CI】【远程监控】$(date '+%F %T') fetch 失败，下轮重试"
    sleep "$INTERVAL"
    continue
  fi

  REMOTE=$(git rev-parse origin/main) || exit 1
  if [ "$REMOTE" != "$BASELINE" ]; then
    echo "【CI】【远程监控】$(date '+%F %T') 检测到新提交: $BASELINE -> $REMOTE"
    git log --oneline "$BASELINE..$REMOTE"
    # 2. 【CI】【远程监控】快进到远程提交
    if git merge --ff-only origin/main; then
      echo "【CI】【远程监控】$(date '+%F %T') 开始编译验证"
      # 3. 【CI】【远程监控】保留完整诊断，管道退出状态包含编译器失败
      if cargo check --locked --workspace --all-targets 2>&1 | tee scripts/.remote-monitor-build.log | tail -n 20; then
        echo "【CI】【远程监控】$(date '+%F %T') 编译通过: $REMOTE"
      else
        echo "【CI】【远程监控】$(date '+%F %T') 编译失败: $REMOTE"
        echo "【CI】【远程监控】完整编译日志: scripts/.remote-monitor-build.log"
      fi
      BASELINE=$REMOTE
    else
      echo "【CI】【远程监控】$(date '+%F %T') 无法快进到远程提交，保留基线并在下轮重试"
    fi
  fi
  sleep "$INTERVAL"
done
