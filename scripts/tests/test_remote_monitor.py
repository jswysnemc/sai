"""【构建监控】【回归测试】隔离外部命令，验证轮询结果与完整错误日志。"""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


class RemoteMonitorTests(unittest.TestCase):
    """【构建监控】【隔离执行】每个用例使用独立目录，不接触真实仓库或远程服务。"""

    def setUp(self):
        """建立模拟仓库及命令；无参数，返回值为空。"""
        self.temporary = tempfile.TemporaryDirectory(prefix="sai-monitor-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "scripts").mkdir()
        self.binary = self.root / "bin"
        self.binary.mkdir()
        source = Path(__file__).resolve().parents[1] / "remote-monitor.sh"
        shutil.copyfile(source, self.root / "scripts/remote-monitor.sh")
        self.command("git", """
case "$1" in
    rev-parse)
        if [ -f .seen ]; then printf 'new\\n'; else touch .seen; printf 'old\\n'; fi
        ;;
    fetch)
        if [ "${TEST_FETCH_STATUS:-0}" != 0 ]; then
            printf 'fatal: fixture TLS failure\\n' >&2
            exit "$TEST_FETCH_STATUS"
        fi
        ;;
    merge) exit "${TEST_MERGE_STATUS:-0}" ;;
    log) printf 'new fixture commit\\n' ;;
esac
""")
        self.command("cargo", """
touch .compiled
printf 'fixture compiler diagnostic\\n'
for number in {1..10}; do printf 'compiler detail %s\\n' "$number"; done
exit "${TEST_CARGO_STATUS:-0}"
""")
        self.command("sleep", 'kill -TERM "$PPID"\n')

    def command(self, name, body):
        """写入一个模拟命令；参数为名称与正文，返回值为空。"""
        path = self.binary / name
        path.write_text("#!/usr/bin/env bash\n" + body)
        path.chmod(0o755)

    def run_monitor(self, **settings):
        """执行一轮监控；参数为故障设置，返回合并输出。"""
        environment = {**os.environ, "PATH": f"{self.binary}{os.pathsep}{os.environ['PATH']}", **settings}
        result = subprocess.run(
            ["bash", str(self.root / "scripts/remote-monitor.sh"), "1"],
            env=environment, capture_output=True, text=True, timeout=5,
        )
        return result.stdout + result.stderr

    def test_compile_failure_is_reported_and_diagnostics_are_retained(self):
        """编译退出非零时报告失败并保留首条诊断；无参数，返回值为空。"""
        output = self.run_monitor(TEST_CARGO_STATUS="101")
        self.assertIn("编译失败: new", output)
        self.assertNotIn("编译通过", output)
        log = self.root / "scripts/.remote-monitor-build.log"
        self.assertIn("fixture compiler diagnostic", log.read_text())

    def test_successful_compilation_is_reported(self):
        """编译成功时报告对应提交；无参数，返回值为空。"""
        self.assertIn("编译通过: new", self.run_monitor())

    def test_failed_fetch_does_not_compile_stale_sources(self):
        """获取远程引用失败时留待重试；无参数，返回值为空。"""
        output = self.run_monitor(TEST_FETCH_STATUS="1")
        self.assertIn("fetch 失败", output)
        self.assertFalse((self.root / ".compiled").exists())
        self.assertIn("fixture TLS failure", (self.root / "scripts/.remote-monitor.err").read_text())

    def test_failed_merge_does_not_compile(self):
        """无法快进时保留工作区；无参数，返回值为空。"""
        self.run_monitor(TEST_MERGE_STATUS="1")
        self.assertFalse((self.root / ".compiled").exists())


if __name__ == "__main__":
    unittest.main()
