"""【终端】【历史排版回归】通过真实工具保存历史，再恢复并检查代码续行。"""

import hashlib
import subprocess
import unittest

from mock_activity_model import ActivityModel
from terminal import TerminalSession


class ActivityResumeRenderingTests(unittest.TestCase):
    """【终端】【恢复排版】使用隔离文件和本地模型，不读取用户会话。"""

    def check_restored_code(self, tool, arguments, markers):
        """参数 tool/arguments 为工具调用、markers 为正文字符；验证恢复排版，返回无。"""
        launch = []

        def prepare(root, environment, binary):
            """参数为隔离目录、环境和程序；通过真实 CLI 创建历史并设置恢复参数，返回无。"""
            (root / "fixture.rs").write_text("\n" * 119 + "        " + "x" * 90 + "\n")
            result = subprocess.run([str(binary), "ask", "Inspect saved code"], cwd=root,
                                    env=environment, capture_output=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stderr.decode())
            scope = hashlib.sha256(str(root.resolve()).encode()).hexdigest()[:16]
            session = (root / "state/sai/sessions/workspaces" / scope / "current").read_text().strip()
            launch.extend(["resume", session, "--lang", "en-US"])

        with ActivityModel(tool, arguments) as model:
            model.advance.set()
            with TerminalSession(model.config(), columns=55, rows=40, prepare=prepare,
                                 arguments=launch, ready=lambda t: "ANSWER-END" in t.text()) as terminal:
                rows = terminal.text().splitlines()
                for marker in markers:
                    first = next(index for index, line in enumerate(rows)
                                 if line.lstrip().startswith("120 ") and marker * 4 in line)
                    column = rows[first].index(marker * 4)
                    self.assertEqual(rows[first + 1].index(marker * 4), column, terminal.text())
                if tool == "read_file":
                    self.assertIn("1 read · \uf0eb 1 thought", terminal.text())

    def test_resume_read_file_preserves_source_indent(self):
        """无参数；真实读取历史恢复后，续行与代码起始列相同，返回无。"""
        self.check_restored_code("read_file", {"path": "fixture.rs", "offset": 120, "limit": 1}, "x")

    def test_resume_diff_preserves_source_indent(self):
        """无参数；真实补丁历史恢复后，增删行续行均与代码起始列相同，返回无。"""
        self.check_restored_code("str_replace", {"path": "fixture.rs",
            "old_string": "x" * 90, "new_string": "y" * 90}, "xy")


if __name__ == "__main__":
    unittest.main()
