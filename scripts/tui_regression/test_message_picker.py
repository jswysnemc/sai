"""【消息导航】【终端回归】标题展开用户消息列表，选择后跳转，取消后保留草稿。"""

import unittest

from terminal import TerminalSession
from test_fullscreen import TurnModel, click, lines


class MessagePickerTests(unittest.TestCase):
    """通过真实终端事件验证消息选择和视图恢复。"""

    def test_click_select_resize_cancel_and_continue(self):
        """无参数；鼠标与键盘均可定位消息，缩放和取消不改写输入草稿；返回无。"""
        with TurnModel() as model, TerminalSession(
            model.config(), columns=100, rows=30, arguments=["--lang", "en-US"]
        ) as terminal:
            for name in ["ALPHA", "BRAVO", "CHARLIE"]:
                terminal.send(name.encode() + b"\r")
                terminal.wait_for(lambda: f"ANSWER-{name}-7" in terminal.text(), timeout=15)
            terminal.send(b"\x0f")
            terminal.wait_for(lambda: "3/3" in lines(terminal)[0])
            terminal.send(b"KEPT-DRAFT")
            terminal.wait_for(lambda: "KEPT-DRAFT" in terminal.text())
            # 1. 【消息导航】【鼠标跳转】展开列表后选择首条，面板关闭并跳到原消息
            click(terminal, 3, 0)
            terminal.wait_for(lambda: "User messages (3)" in terminal.text())
            for index, name in enumerate(["ALPHA", "BRAVO", "CHARLIE"], 1):
                self.assertIn(f"{index}  {name}", terminal.text())
            row = next(i for i, line in enumerate(lines(terminal)) if "1  ALPHA" in line)
            click(terminal, 5, row)
            terminal.wait_for(lambda: "1/3" in lines(terminal)[0] and "User messages" not in terminal.text())
            self.assertIn("KEPT-DRAFT", terminal.text())
            # 2. 【消息导航】【键盘跳转】焦点移动和确认不提交输入框中的草稿
            click(terminal, 3, 0)
            terminal.wait_for(lambda: "User messages (3)" in terminal.text())
            terminal.send(b"\x1b[B\r")
            terminal.wait_for(lambda: "2/3" in lines(terminal)[0] and "User messages" not in terminal.text())
            # 3. 【消息导航】【缩放取消】窄屏保持列表可见，Esc 恢复当前会话及草稿
            click(terminal, 3, 0)
            terminal.wait_for(lambda: "User messages (3)" in terminal.text())
            terminal.resize(42, 18)
            terminal.wait_for(lambda: "User messages (3)" in terminal.text())
            terminal.send(b"\x1b")
            terminal.wait_for(lambda: "User messages" not in terminal.text())
            self.assertIn("KEPT-DRAFT", terminal.text())
            self.assertIn("2/3", lines(terminal)[0])
            terminal.resize(100, 30)
            terminal.pump(.3)
            terminal.send(b"\x7f" * len("KEPT-DRAFT") + b"DELTA\r")
            terminal.send(b"\x1b[1;5B")
            terminal.wait_for(lambda: "ANSWER-DELTA-7" in terminal.text(), timeout=15)
            terminal.wait_for(lambda: "4/4" in lines(terminal)[0])


if __name__ == "__main__":
    unittest.main()
