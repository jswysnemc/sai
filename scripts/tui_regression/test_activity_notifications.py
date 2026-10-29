"""【终端】【展示细节回归】验证固定状态、悬停按钮和 SVG 图像占位。"""

import re
import threading
import unittest

from kitty_assertions import placements
from mock_activity_model import ActivityModel
from terminal import TerminalSession
from test_fullscreen import click, lines
from test_fullscreen_parts import idle


class ActivityNotificationTests(unittest.TestCase):
    """用隔离的伪终端检查固定状态及图像与文本的实际相对位置。"""

    def row_of(self, terminal, needle):
        """参数 terminal 为终端、needle 为标记；返回首个匹配行号。"""
        return next(row for row, line in enumerate(lines(terminal)) if needle in line)

    def test_background_completion_does_not_add_a_navigation_point(self):
        """无参数；真实后台完成回执进入工具展示，导航总数只计算用户提交。"""
        with ActivityModel("background_command", {"action": "start", "command": "sleep 1; printf BACKGROUND-DONE"}) as model:
            model.advance.set()
            with TerminalSession(model.config(), columns=100, rows=40,
                    arguments=["--lang", "zh-CN"]) as terminal:
                terminal.send(b"Inspect background notification\r")
                terminal.wait_for(lambda: "Background command completed" in terminal.text(), timeout=20)
                terminal.pump(.4)
                counter = re.search(r"\b(\d+)/(\d+)\b", lines(terminal)[0])
                self.assertIsNotNone(counter)
                self.assertEqual(counter.group(2), "1")
                self.assertNotIn("后台工作已完成", terminal.text())
                self.assertNotIn("状态：", terminal.text())

    def test_queued_delivery_is_a_tool_without_a_second_navigation_point(self):
        """无参数；排队消息在模型请求间隙交付时显示工具通知，不计为新的用户导航点。"""
        with ActivityModel("read_file", {"path": "fixture.txt"}) as model:
            with TerminalSession(model.config(), columns=100, rows=40,
                    arguments=["--lang", "en-US"]) as terminal:
                (terminal.root / "fixture.txt").write_text("fixture")
                terminal.send(b"INITIAL-USER\r")
                terminal.wait_for(model.ready.is_set)
                terminal.send(b"FOLLOW-UP-CONTENT\r")
                terminal.wait_for(lambda: "FOLLOW-UP-CONTENT" in terminal.text())
                terminal.send(b"\x1b[1;5A")
                terminal.pump(.2)
                terminal.send(b"\t")
                terminal.wait_for(lambda: "request" in terminal.text())
                terminal.send(b"\x1b")
                terminal.pump(.2)
                model.advance.set()
                terminal.wait_for(lambda: "Queued message delivered" in terminal.text(), timeout=20)
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text())
                terminal.pump(.3)
                counter = re.search(r"\b(\d+)/(\d+)\b", lines(terminal)[0])
                self.assertIsNotNone(counter)
                self.assertEqual(counter.group(2), "1")
                self.assertIn("FOLLOW-UP-CONTENT", terminal.text())

    def test_waiting_response_stays_above_input_during_scroll_and_resize(self):
        """无参数；等待模型响应时，状态不随历史滚动，缩放后仍紧邻输入区。"""
        with ActivityModel("read_file", {"path": "fixture.txt"}) as model:
            model.advance.set()
            model.answer_gate = threading.Event()
            with TerminalSession(model.config(), columns=100, rows=32,
                    arguments=["--lang", "zh-CN"]) as terminal:
                (terminal.root / "fixture.txt").write_text("\n".join(f"LINE-{index}" for index in range(40)))
                terminal.send(b"Inspect fixture\r")
                terminal.wait_for(model.answer_ready.is_set)
                terminal.wait_for(lambda: "Waiting for response" in terminal.text())
                for columns, rows in [(100, 32), (55, 24), (80, 30)]:
                    terminal.resize(columns, rows)
                    terminal.pump(.4)
                    terminal.send(b"\x1b[5~")
                    terminal.pump(.2)
                    status = self.row_of(terminal, "Waiting for response")
                    prompt = self.row_of(terminal, "→")
                    self.assertGreater(prompt, status)
                    self.assertLessEqual(prompt - status, 3)
                    self.assertEqual(terminal.text().count("Waiting for response"), 1)

    def test_hover_uses_disclosure_and_preserves_child_alignment(self):
        """无参数；分组子标题同层对齐，悬停按展开状态显示 ▾/▸，移开恢复语义图标。"""
        with ActivityModel("read_file", {"path": "fixture.txt"}) as model:
            model.advance.set()
            with TerminalSession(model.config(), columns=100, rows=32,
                    arguments=["--lang", "en-US"]) as terminal:
                (terminal.root / "fixture.txt").write_text("FILE-CONTENT")
                terminal.send(b"Inspect fixture\r")
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text())
                group = self.row_of(terminal, "1 read · \uf0eb 1 thought")
                thought = self.row_of(terminal, "Thought")
                tool = self.row_of(terminal, "Read file")
                group_column = len(lines(terminal)[group]) - len(lines(terminal)[group].lstrip())
                thought_column = len(lines(terminal)[thought]) - len(lines(terminal)[thought].lstrip())
                tool_column = len(lines(terminal)[tool]) - len(lines(terminal)[tool].lstrip())
                self.assertEqual(thought_column, group_column + 2)
                self.assertEqual(tool_column, thought_column)
                self.assertEqual(group_column, 0)
                terminal.send(f"\x1b[<35;{tool_column + 1};{tool + 1}M".encode())
                terminal.wait_for(lambda: lines(terminal)[tool].lstrip().startswith("▾ Read file"))
                click(terminal, tool_column + 1, tool)
                terminal.wait_for(lambda: "FILE-CONTENT" not in terminal.text())
                terminal.wait_for(lambda: lines(terminal)[tool].lstrip().startswith("▸ Read file"))
                click(terminal, tool_column + 1, tool)
                terminal.wait_for(lambda: "FILE-CONTENT" in terminal.text())
                terminal.wait_for(lambda: lines(terminal)[tool].lstrip().startswith("▾ Read file"))
                prompt = self.row_of(terminal, "→")
                terminal.send(f"\x1b[<35;1;{prompt + 1}M".encode())
                terminal.wait_for(lambda: lines(terminal)[tool].lstrip().startswith("\uf15c Read file"))

    def test_svg_preserves_space_before_following_summary(self):
        """无参数；两种布局的 SVG 尾部保留协议占位，摘要不会落在图像内部。"""
        for style in ["inline", "fullscreen"]:
            with self.subTest(style=style), ActivityModel("read_file", {"path": "fixture.txt"}) as model:
                model.advance.set()
                model.answer = ('SVG-HEAD\n```svg\n<svg xmlns="http://www.w3.org/2000/svg" '
                    'width="640" height="240"><rect width="640" height="240" fill="#1475aa"/></svg>\n```\n')
                config = model.config()
                config["display"]["fullscreen"] = True
                with TerminalSession(config, columns=100, rows=44,
                        arguments=["--lang", "en-US", "--view", style], kitty_graphics=True) as terminal:
                    (terminal.root / "fixture.txt").write_text("fixture")
                    terminal.send(b"Inspect SVG spacing\r")
                    terminal.wait_for(lambda: b"\x1b_Ga=p" in terminal.raw)
                    terminal.wait_for(lambda: idle(terminal))
                    terminal.pump(.3)
                    image_row, image_column, image_height = placements(bytes(terminal.raw), 100, 44)[-1]
                    self.assertLessEqual(image_column, 4)
                    summary = next(row for row, line in enumerate(lines(terminal))
                        if row >= image_row and ("Worked for" in line or "Context" in line))
                    self.assertGreaterEqual(summary, image_row + image_height)
                    self.assertLess(summary, self.row_of(terminal, "→"))


if __name__ == "__main__":
    unittest.main()
