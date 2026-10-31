"""【终端侧栏】【真实回归】宽屏显示概览，Git 变化异步刷新，缩放不丢失输入草稿。"""

import subprocess
import unittest

from terminal import TerminalSession
from mock_activity_model import ActivityModel
from test_fullscreen import TurnModel, click, lines


class SidebarTests(unittest.TestCase):
    """使用隔离工作树验证响应式全屏侧栏。"""

    def test_todo_sidebar_click_and_narrow_fallback(self):
        """无参数；待办在侧栏展开，缩窄后回到输入框上方，返回无。"""
        texts = ["CURRENT-TASK", *[f"NEXT-TASK-{index}" for index in range(4)]]
        with ActivityModel("todo", {"action": "add", "texts": texts}) as model:
            model.advance.set()
            with TerminalSession(model.config(), columns=160, rows=32,
                                 arguments=["--lang", "en-US"]) as terminal:
                terminal.send(b"Inspect sidebar tasks\r")
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text(), timeout=15)
                terminal.wait_for(lambda: any("CURRENT-TASK" in line[120:] for line in lines(terminal)))
                row = next(index for index, line in enumerate(lines(terminal)) if "Todo" in line[120:])
                # 1. 【终端侧栏】【整行点击】标题右侧留白与文字具有相同操作
                terminal.send(f"\x1b[<35;157;{row + 1}M".encode())
                terminal.wait_for(lambda: "Click to show all tasks" in terminal.text())
                click(terminal, 156, row)
                terminal.wait_for(lambda: any("NEXT-TASK-2" in line[120:] for line in lines(terminal)))
                row = next(index for index, line in enumerate(lines(terminal)) if "NEXT-TASK-2" in line[120:])
                click(terminal, 156, row, kind=65)
                terminal.wait_for(lambda: any("NEXT-TASK-3" in line[120:] for line in lines(terminal)))
                terminal.send(b"KEPT-DRAFT")
                terminal.resize(100, 32)
                terminal.wait_for(lambda: "Changes" not in terminal.text())
                terminal.wait_for(lambda: "CURRENT-TASK" in terminal.text())
                self.assertIn("KEPT-DRAFT", terminal.text())
                terminal.resize(160, 32)
                terminal.wait_for(lambda: "Changes" in terminal.text())
                self.assertIn("KEPT-DRAFT", terminal.text())

    def test_git_refresh_width_fallback_and_message_picker(self):
        """无参数；宽屏读取工作树并定时刷新，窄屏隐藏侧栏，消息列表不覆盖侧栏；返回无。"""
        def prepare(root, environment, binary):
            """参数为隔离目录、环境与程序；初始化 Git 仓库，返回无。"""
            for args in [["init"], ["config", "user.name", "Fixture"],
                         ["config", "user.email", "fixture@example.invalid"]]:
                subprocess.run(["git", *args], cwd=root, check=True, capture_output=True)
            (root / "tracked.txt").write_text("before\n")
            (root / ".gitignore").write_text("config/\ndata/\nstate/\ncache/\npictures/\n")
            subprocess.run(["git", "add", "tracked.txt", ".gitignore"], cwd=root, check=True, capture_output=True)
            subprocess.run(["git", "commit", "-m", "test: sidebar fixture"],
                           cwd=root, check=True, capture_output=True)

        with TurnModel() as model:
            config = model.config()
            config["display"]["fullscreen"] = True
            with TerminalSession(config, columns=160, rows=32, prepare=prepare,
                                 arguments=["--lang", "en-US"]) as terminal:
                terminal.wait_for(lambda: "Working tree clean" in terminal.text())
                for heading in ["Subagents", "Changes", "Todo"]:
                    self.assertIn(heading, terminal.text())
                terminal.send(b"ALPHA\r")
                terminal.wait_for(lambda: "ANSWER-ALPHA-7" in terminal.text(), timeout=15)
                terminal.send(b"KEPT-DRAFT")
                terminal.wait_for(lambda: "KEPT-DRAFT" in terminal.text())
                # 1. 【终端侧栏】【异步刷新】外部修改在空闲时显示，不依赖用户提交消息
                (terminal.root / "tracked.txt").write_text("after\n")
                terminal.wait_for(lambda: "tracked.txt" in terminal.text(), timeout=8)
                (terminal.root / "new-file.txt").write_text("untracked\n")
                terminal.wait_for(lambda: "new-file.txt" in terminal.text(), timeout=8)
                self.assertIn("+", terminal.text())
                self.assertIn("-", terminal.text())
                # 2. 【终端侧栏】【分组折叠】悬停解释动作，折叠后仍异步更新变更数量
                row = next(index for index, line in enumerate(lines(terminal)) if "Changes" in line[120:])
                terminal.send(f"\x1b[<35;157;{row + 1}M".encode())
                terminal.wait_for(lambda: "Click to collapse" in terminal.text())
                click(terminal, 156, row)
                terminal.wait_for(lambda: "tracked.txt" not in terminal.text()
                                  and "new-file.txt" not in terminal.text())
                terminal.send(f"\x1b[<35;157;{row + 1}M".encode())
                terminal.wait_for(lambda: "Click to expand" in terminal.text())
                (terminal.root / "folded-file.txt").write_text("folded\n")
                terminal.wait_for(lambda: any("Changes" in line[120:] and "3 " in line[120:]
                                             for line in lines(terminal)), timeout=8)
                self.assertNotIn("folded-file.txt", terminal.text())
                click(terminal, 156, row)
                terminal.wait_for(lambda: "folded-file.txt" in terminal.text())
                (terminal.root / "folded-file.txt").unlink()
                terminal.wait_for(lambda: "folded-file.txt" not in terminal.text(), timeout=8)
                # 3. 【终端侧栏】【消息面板】顶部列表只覆盖正文列，右侧概览仍可见
                click(terminal, 3, 0)
                terminal.wait_for(lambda: "User messages (1)" in terminal.text())
                self.assertIn("Subagents", lines(terminal)[3])
                self.assertIn("Changes", terminal.text())
                terminal.send(b"\x1b")
                terminal.wait_for(lambda: "User messages" not in terminal.text())
                # 4. 【终端侧栏】【宽窄回退】正文重新排版，输入草稿保持不变
                terminal.resize(100, 32)
                terminal.wait_for(lambda: "Changes" not in terminal.text())
                self.assertIn("KEPT-DRAFT", terminal.text())
                terminal.resize(140, 32)
                terminal.wait_for(lambda: "Changes" in terminal.text())
                self.assertIn("KEPT-DRAFT", terminal.text())
                # 5. 【终端侧栏】【干净状态】移除文件和恢复内容后显示当前真实工作树状态
                (terminal.root / "new-file.txt").unlink()
                (terminal.root / "tracked.txt").write_text("before\n")
                terminal.wait_for(lambda: "tracked.txt" not in terminal.text()
                                  and "new-file.txt" not in terminal.text(), timeout=8)


if __name__ == "__main__":
    unittest.main()
