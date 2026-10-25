"""【终端】【展示回归】检查输入与用户消息回滚，以及中文环境中的英文界面文案。"""

import unittest

from mock_activity_model import ActivityModel
from mock_tool_model import MockToolModel
from terminal import TerminalSession
from test_fullscreen import click, lines


class DeepseekPresentationTests(unittest.TestCase):
    """通过隔离终端保留中文消息内容，同时验证英文提示和共用输入框。"""

    def test_shared_input_and_user_message_with_english_chrome(self):
        """无参数；中文环境恢复深色输入框与圆点消息，保留英文摘要及中文文件内容。"""
        with ActivityModel("read_file", {"path": "fixture.txt", "limit": 20}) as model:
            model.advance.set()
            with TerminalSession(model.config(), columns=100, rows=32,
                    arguments=["--lang", "zh-CN"]) as terminal:
                prompt_row = next(row for row, line in enumerate(lines(terminal)) if "→" in line)
                self.assertIn("Enter a message or type /help", lines(terminal)[prompt_row])
                self.assertEqual(terminal.screen.buffer[prompt_row][1].bg, "262626")
                self.assertEqual(terminal.screen.buffer[prompt_row - 1][1].bg, "262626")
                self.assertEqual(terminal.screen.buffer[prompt_row + 1][1].bg, "262626")
                (terminal.root / "fixture.txt").write_text("原始文件内容\nsecond\n")
                terminal.send("用户消息\r".encode())
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text(), timeout=15)
                terminal.wait_for(lambda: "Thought" in terminal.text())
                self.assertIn("● 用户消息", terminal.text())
                self.assertNotIn("› 用户消息", terminal.text())
                self.assertIn("1 tool · 1 thought", terminal.text())
                self.assertIn("Read file", terminal.text())
                self.assertIn("Ctrl+O exit", terminal.text())
                self.assertNotIn("已思考", terminal.text())
                self.assertNotIn("读取文件", terminal.text())
                terminal.wait_for(lambda: "原始文件内容" in terminal.text())
                terminal.send(b"/style deepseek\r")
                terminal.wait_for(lambda: "terminal style: deepseek" in terminal.text())
                self.assertNotIn("终端样式", terminal.text())

    def test_working_prompt_and_scroll_hints_stay_english(self):
        """无参数；中文环境下的排队提示、快捷键与恢复跟随提示均使用英文。"""
        with MockToolModel("run_command") as model:
            config = model.config()
            config["display"] = {"tui_style": "deepseek"}
            with TerminalSession(config, columns=100, rows=32,
                    arguments=["--lang", "zh-CN"]) as terminal:
                terminal.send("用户任务\r".encode())
                terminal.wait_for(model.ready.is_set)
                model.fragments.put('{"command":"\n' + "\n".join(f"DETAIL-{index:02}" for index in range(40)) + "\n")
                terminal.wait_for(lambda: "DETAIL-39" in terminal.text())
                self.assertIn("Preparing", terminal.text())
                self.assertIn("Add a follow-up to the queue", terminal.text())
                self.assertIn("Following latest", terminal.text())
                self.assertIn("Ctrl+C stop", terminal.text())
                terminal.send(b"DRAFT")
                terminal.wait_for(lambda: "DRAFT" in terminal.text())
                terminal.wait_for(lambda: "Enter queue message" in terminal.text())
                self.assertNotIn("排队发送", terminal.text())
                self.assertNotIn("换行", terminal.text())
                terminal.send(b"\x1b[5;3~")
                terminal.wait_for(lambda: "Click to follow latest" in terminal.text())
                self.assertNotIn("跟随最新步骤", terminal.text())
                self.assertNotIn("点击恢复跟随", terminal.text())
                self.assertNotIn("输入后续消息", terminal.text())


if __name__ == "__main__":
    unittest.main()
