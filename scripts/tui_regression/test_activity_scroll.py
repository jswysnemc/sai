"""【终端】【滚动体验回归】验证链内上下文、键盘导航和完成后的紧凑布局。"""

import re
import unittest

from mock_activity_model import ActivityModel
from mock_tool_model import MockToolModel
from terminal import TerminalSession
from test_fullscreen import TurnModel, click, lines


class ActivityScrollTests(unittest.TestCase):
    """用真实伪终端验证滚动体验，不依赖外部服务。"""

    def row_of(self, terminal, text):
        """参数 terminal 为终端，text 为标记；返回匹配行号。"""
        return next(index for index, line in enumerate(lines(terminal)) if text in line)

    def start_details(self, model, terminal):
        """参数 model 为模型替身，terminal 为终端；输出长详情并等待显示，返回无。"""
        terminal.send(b"Inspect detailed output\r")
        terminal.wait_for(model.ready.is_set)
        model.fragments.put('{"command":"\n' + "\n".join(f"DETAIL-{index:02}" for index in range(40)) + "\n")
        terminal.wait_for(lambda: "DETAIL-39" in terminal.text())

    def test_keyboard_navigation_keeps_tool_context_and_composer(self):
        """无参数；链内翻页和首尾定位始终保留工具标题，不移动输入区。"""
        with MockToolModel("run_command") as model:
            config = model.config()
            config["display"] = {"fullscreen": True}
            with TerminalSession(config, columns=100, rows=30,
                    arguments=["--lang", "en-US"]) as terminal:
                self.start_details(model, terminal)
                self.assertIn("Preparing", terminal.text())
                group = self.row_of(terminal, "Preparing")
                composer = self.row_of(terminal, "→")
                terminal.send(b"\x1b[5;3~")
                terminal.wait_for(lambda: "DETAIL-39" not in terminal.text())
                self.assertIn("Preparing", terminal.text())
                self.assertEqual(self.row_of(terminal, "Preparing"), group)
                self.assertEqual(self.row_of(terminal, "→"), composer)
                terminal.send(b"\x1b[1;3H")
                terminal.wait_for(lambda: "DETAIL-00" in terminal.text())
                terminal.send(b"\x1b[1;3F")
                terminal.wait_for(lambda: "DETAIL-39" in terminal.text())
                self.assertIn("Following latest", terminal.text())
                self.assertEqual(self.row_of(terminal, "→"), composer)

    def test_click_to_resume_and_fold_pinned_title(self):
        """无参数；提示行恢复跟随，保留标题可折叠，当前步骤重新打开时显示最新详情。"""
        with MockToolModel("run_command", reasoning="Inspect the command output") as model:
            config = model.config()
            config["display"] = {"fullscreen": True}
            with TerminalSession(config, columns=100, rows=30,
                    arguments=["--lang", "en-US"]) as terminal:
                self.start_details(model, terminal)
                terminal.send(b"\x1b[5;3~")
                terminal.wait_for(lambda: "Click to follow latest" in terminal.text())
                click(terminal, 20, self.row_of(terminal, "Click to follow latest"))
                terminal.wait_for(lambda: "DETAIL-39" in terminal.text())
                click(terminal, 6, self.row_of(terminal, "Preparing"))
                terminal.wait_for(lambda: "DETAIL-39" not in terminal.text())
                self.assertIn("Preparing", terminal.text())
                click(terminal, 6, self.row_of(terminal, "Preparing"))
                terminal.wait_for(lambda: "DETAIL-39" in terminal.text())
                self.assertIn("Following latest", terminal.text())

    def test_completed_short_chain_does_not_keep_empty_viewport(self):
        """无参数；已完成的短链恢复紧凑布局，不显示空视口与冗余提示。"""
        with ActivityModel("read_file", {"path": "fixture.txt", "offset": 1, "limit": 3}) as model:
            model.advance.set()
            with TerminalSession(model.config(), columns=100, rows=30,
                    arguments=["--lang", "en-US"]) as terminal:
                (terminal.root / "fixture.txt").write_text("one\ntwo\nthree")
                terminal.send(b"Read fixture.txt\r")
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text(), timeout=15)
                terminal.pump(.3)
                group = self.row_of(terminal, "\uf15c 1 read · \uf0eb 1 thought")
                answer = self.row_of(terminal, "ANSWER-END")
                self.assertLessEqual(answer - group, 10)
                self.assertIn("Thought", terminal.text())
                self.assertIn("Read file", terminal.text())
                self.assertNotIn("Following latest", terminal.text())

    def test_folding_scrolled_history_keeps_group_position(self):
        """无参数；长会话中收起滚动后的节点，不把分组标题向上移动。"""
        with ActivityModel("read_file", {"path": "fixture.txt"}) as model:
            config = model.config()
            config["display"]["fullscreen"] = True
            model.advance.set()
            with TerminalSession(config, columns=100, rows=30,
                    arguments=["--lang", "en-US"]) as terminal:
                (terminal.root / "fixture.txt").write_text("fixture content")
                for name in ["ALPHA", "BRAVO"]:
                    model.reasoning = "".join(f"THINK-{name}-{index}\n" for index in range(40))
                    model.answer = f"ANSWER-{name}-7"
                    terminal.send((name + "\r").encode())
                    terminal.wait_for(lambda: f"ANSWER-{name}-7" in terminal.text(), timeout=15)
                    terminal.pump(.3)
                terminal.send(b"\x1b[1;5H")
                terminal.wait_for(lambda: "Thought" in terminal.text())
                group = self.row_of(terminal, "1 read · \uf0eb 1 thought")
                terminal.send(b"\x1b[1;3H")
                terminal.wait_for(lambda: "THINK-ALPHA-0" in terminal.text())
                terminal.send(f"\x1b[<65;8;{group + 5}M".encode())
                terminal.wait_for(lambda: "THINK-ALPHA-3" in terminal.text() and "THINK-ALPHA-0" not in terminal.text())
                click(terminal, 6, self.row_of(terminal, "Thought"))
                terminal.wait_for(lambda: "THINK-ALPHA-" not in terminal.text())
                terminal.pump(.2)
                self.assertEqual(self.row_of(terminal, "1 read · \uf0eb 1 thought"), group)

    def test_resize_preserves_paused_reading_content(self):
        """无参数；暂停浏览后往返缩放保留同一内容，不跳到早期输出或恢复尾部跟随。"""
        with MockToolModel("run_command") as model:
            config = model.config()
            config["display"] = {"fullscreen": True}
            with TerminalSession(config, columns=100, rows=30,
                    arguments=["--lang", "en-US"]) as terminal:
                terminal.send(b"Inspect wrapped details\r")
                terminal.wait_for(model.ready.is_set)
                model.fragments.put('{"command":"\n' + "\n".join(
                    f"READ-{index:02} " + "abcdefghij " * 7 for index in range(30)) + "\n")
                terminal.wait_for(lambda: "READ-29" in terminal.text())
                terminal.send(b"\x1b[5;3~")
                terminal.wait_for(lambda: "READ-29" not in terminal.text())
                marker = min(re.findall(r"READ-\d+", terminal.text()))
                for columns, rows in [(50, 30), (80, 20), (100, 30)]:
                    before = len(terminal.raw)
                    terminal.resize(columns, rows)
                    terminal.wait_for(lambda: len(terminal.raw) > before + 500)
                    terminal.pump(.2)
                    self.assertIn(marker, terminal.text())
                    self.assertNotIn("READ-29", terminal.text())
                    self.assertIn("Preparing", terminal.text())

    def test_resize_preserves_historical_file_position(self):
        """无参数；历史文件的语法颜色与行号不影响缩放后的阅读定位。"""
        with ActivityModel("read_file", {"path": "fixture.rs", "offset": 1, "limit": 40}) as model:
            model.advance.set()
            with TerminalSession(model.config(), columns=100, rows=30,
                    arguments=["--lang", "en-US"]) as terminal:
                (terminal.root / "fixture.rs").write_text("\n".join(
                    f'pub const MARKER_{index:02}: &str = "' + "abcdefghij " * 8 + '";'
                    for index in range(40)))
                terminal.send(b"Read fixture.rs\r")
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text(), timeout=15)
                terminal.send(b"\x1b[1;3H")
                terminal.wait_for(lambda: "MARKER_00" in terminal.text())
                terminal.send(b"\x1b[6;3~")
                terminal.pump(.3)
                marker = min(re.findall(r"MARKER_\d+", terminal.text()))
                self.assertNotEqual(marker, "MARKER_00")
                for columns in [50, 80, 100]:
                    before = len(terminal.raw)
                    terminal.resize(columns, 30)
                    terminal.wait_for(lambda: len(terminal.raw) > before + 500)
                    terminal.pump(.1)
                    self.assertIn(marker, terminal.text())
                    self.assertIn("Read file", terminal.text())


if __name__ == "__main__":
    unittest.main()
