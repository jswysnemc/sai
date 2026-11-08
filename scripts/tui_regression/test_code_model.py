"""【会话模式】【终端回归】验证真实按键和命令在空闲及生成期间切换脚本模式。"""

import unittest

from mock_model import MockModel
from terminal import TerminalSession


class CodeModelTests(unittest.TestCase):
    """覆盖模式循环、斜杠命令及生成期间的热切换。"""

    def test_shift_tab_and_slash_command_enter_code_model(self):
        """【会话模式】【空闲切换】参数为测试实例；返回无，检查快捷键及命令实际显示。"""
        with TerminalSession(arguments=["--yolo"]) as terminal:
            terminal.send(b"/code-model\r")
            terminal.wait_for(lambda: "code-model" in terminal.text().lower())
            for _ in range(3):
                terminal.send(b"\x1b[Z")
            terminal.wait_for(lambda: "yolo+code-model" in terminal.text().lower())
            terminal.send(b"/yolo\r")
            terminal.pump(.3)
            terminal.send(b"/code-model on\r")
            terminal.wait_for(lambda: "CODE-MODEL" in terminal.text())

    def test_code_model_command_works_during_generation(self):
        """【会话模式】【流式切换】参数为测试实例；返回无，检查命令不作为普通提问排队。"""
        with MockModel(15) as model, TerminalSession(model.config(), arguments=["--yolo"]) as terminal:
            terminal.send(b"Output a table")
            terminal.pump(.2)
            terminal.send(b"\r")
            terminal.wait_for(model.ready.is_set)
            terminal.send(b"/code-model\r")
            terminal.wait_for(lambda: "CODE-MODEL" in terminal.text())
            model.release.set()
            terminal.wait_for(lambda: "AFTER-TABLE" in terminal.text())
