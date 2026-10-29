#!/usr/bin/env bash
set -e

# 进入 sai 源码工程目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

MODE="${1:-all}"

if [ "$MODE" = "compare" ] || [ "$MODE" = "diff" ]; then
    echo "============================================================"
    echo " 运行 sai 工具卡新旧优化方案对比 Demo"
    echo "============================================================"
    # 绕过 rtk 终端输出压缩，直接执行编译好的单元测试输出 ANSI 流
    TEST_BIN=$(ls -t target/debug/deps/sai-* 2>/dev/null | grep -v '\.d$' | head -n 1)
    if [ -n "$TEST_BIN" ] && [ -x "$TEST_BIN" ]; then
        "$TEST_BIN" test_comparison_demo --nocapture
    else
        cargo test test_comparison_demo -- --nocapture
    fi
else
    echo "============================================================"
    echo " 运行 sai 全量工具卡 (26 种) 真实终端渲染 Demo"
    echo "============================================================"
    TEST_BIN=$(ls -t target/debug/deps/sai-* 2>/dev/null | grep -v '\.d$' | head -n 1)
    if [ -n "$TEST_BIN" ] && [ -x "$TEST_BIN" ]; then
        "$TEST_BIN" test_all_tool_cards_demo --nocapture
    else
        cargo test test_all_tool_cards_demo -- --nocapture
    fi
fi
