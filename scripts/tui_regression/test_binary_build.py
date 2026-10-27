"""【终端】【测试程序回归】检查默认构建与显式程序选择，不调用真实编译器。"""

import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from subprocess import CompletedProcess

from binary_build import build_default_binary, resolve_test_binary


class BinaryBuildTests(unittest.TestCase):
    """【终端】【构建选择】确保默认测试不会悄悄复用陈旧程序。"""

    def tearDown(self):
        """无参数；清除模拟构建缓存，避免影响真实终端用例，返回无。"""
        build_default_binary.cache_clear()

    def test_default_binary_builds_once_and_uses_cargo_target(self):
        """无参数；默认构建一次并使用 Cargo 返回的产物目录，返回无。"""
        build_default_binary.cache_clear()
        with patch("binary_build.subprocess.run") as run:
            run.return_value = CompletedProcess([], 0, json.dumps({"reason": "compiler-artifact",
                "target": {"name": "sai"}, "executable": "/tmp/custom-target/debug/sai"}), "")
            first = build_default_binary()
            self.assertEqual(first.parent, Path("/tmp/custom-target/debug"))
            self.assertEqual(build_default_binary(), first)
            self.assertEqual(run.call_count, 1)
            self.assertEqual(run.call_args_list[0].args[0],
                             ["cargo", "build", "--locked", "--bin", "sai", "--message-format=json"])

    def test_explicit_binary_does_not_build(self):
        """无参数；显式程序保持原有选择，便于验证安装版或复现旧版问题，返回无。"""
        with tempfile.TemporaryDirectory() as root:
            binary = Path(root) / "sai"
            binary.touch()
            with patch.dict("os.environ", {"SAI_TEST_BINARY": str(binary)}), patch("binary_build.build_default_binary") as build:
                self.assertEqual(resolve_test_binary(), binary.resolve())
                build.assert_not_called()
