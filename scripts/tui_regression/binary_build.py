"""【终端】【测试程序】默认构建当前源码，避免伪终端验证陈旧的调试程序。"""

from functools import lru_cache
import json
import os
from pathlib import Path
import subprocess


@lru_cache(maxsize=1)
def build_default_binary():
    """无参数；每个测试进程只构建一次工作区程序，返回构建产物路径。"""
    project = Path(__file__).resolve().parents[2]
    result = subprocess.run(["cargo", "build", "--locked", "--bin", "sai", "--message-format=json"], cwd=project,
                            capture_output=True, text=True, timeout=300)
    if result.returncode:
        raise RuntimeError("Cannot build current TUI test binary:\n" + result.stderr)
    # 1. 【终端】【产物定位】读取实际可执行文件，兼容自定义输出目录和编译目标
    for line in result.stdout.splitlines():
        artifact = json.loads(line)
        if (artifact.get("reason") == "compiler-artifact"
                and artifact.get("target", {}).get("name") == "sai" and artifact.get("executable")):
            return Path(artifact["executable"])
    raise RuntimeError("Cargo did not report the sai executable")


def resolve_test_binary():
    """无参数；显式程序优先，否则构建当前源码，返回存在的绝对程序路径。"""
    override = os.environ.get("SAI_TEST_BINARY")
    binary = (Path(override) if override else build_default_binary()).resolve()
    if not binary.is_file():
        raise FileNotFoundError(f"TUI test binary does not exist: {binary}")
    return binary
