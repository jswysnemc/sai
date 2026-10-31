"""【终端】【鼠标光标】验证点击定位、中文、折行及流式草稿编辑。"""

import unittest
from wcwidth import wcswidth

from terminal import TerminalSession
from test_fullscreen import click, lines
from test_git_preview import prepare_repository
from mock_activity_model import ActivityModel


def locate(terminal, text):
    """参数为终端与可见标记；返回标记首字符的屏幕列、行。"""
    row = next(index for index, line in enumerate(lines(terminal)) if text in line)
    line = lines(terminal)[row]
    return wcswidth(line[:line.index(text)]), row


class InputMouseTests(unittest.TestCase):
    """通过真实鼠标事件确认输入光标，而不是仅检查绘制状态。"""

    def test_click_positions_cursor_in_unicode_text_and_trailing_space(self):
        """无参数；中文按显示列定位，行尾留白定位到末尾，单击不复制，返回无。"""
        with TerminalSession(columns=80, rows=24, arguments=["--view", "fullscreen", "--lang", "en-US"]) as terminal:
            terminal.send("中文-CLICK-TAIL".encode())
            terminal.wait_for(lambda: "中文-CLICK-TAIL" in terminal.text())
            column, row = locate(terminal, "CLICK")
            click(terminal, column, row)
            terminal.wait_for(lambda: terminal.screen.cursor.x == column and terminal.screen.cursor.y == row)
            terminal.send(b"X")
            terminal.wait_for(lambda: "中文-XCLICK-TAIL" in terminal.text())
            click(terminal, 50, row)
            terminal.pump(.2)
            terminal.send(b"!")
            terminal.wait_for(lambda: "中文-XCLICK-TAIL!" in terminal.text())
            column, row = locate(terminal, "中文")
            click(terminal, column + 1, row)
            terminal.pump(.2)
            terminal.send(b"Y")
            terminal.wait_for(lambda: "Y中文-XCLICK-TAIL!" in terminal.text())
            self.assertNotIn(b"\x1b]52;c;", terminal.raw)

    def test_blank_line_and_soft_wrap_positions_survive_resize(self):
        """无参数；空行和软折行定位到源码位置，缩放后重新按实际坐标点击，返回无。"""
        with TerminalSession(columns=80, rows=26, arguments=["--view", "fullscreen", "--lang", "en-US"]) as terminal:
            text = "FIRST\n\nSECOND-" + "a" * 55 + "WRAPPED-END"
            terminal.send(b"\x1b[200~" + text.encode() + b"\x1b[201~")
            terminal.wait_for(lambda: "SECOND-" in terminal.text())
            _, first_row = locate(terminal, "FIRST")
            click(terminal, 5, first_row + 1)
            terminal.pump(.2)
            terminal.send(b"BLANK")
            terminal.wait_for(lambda: "BLANK" in terminal.text())
            terminal.resize(50, 26)
            terminal.wait_for(lambda: "WRAPPED-END" in terminal.text())
            column, row = locate(terminal, "WRAPPED-END")
            click(terminal, column, row)
            terminal.pump(.2)
            terminal.send(b"INSERT-")
            terminal.wait_for(lambda: "INSERT-WRAPPED-END" in terminal.text())
            self.assertIn("FIRST", terminal.text())
            self.assertIn("BLANK", terminal.text())

    def test_mouse_cursor_edits_draft_while_model_is_streaming(self):
        """无参数；模型输出期间点击仍修改运行中草稿光标，返回无。"""
        with ActivityModel() as model:
            with TerminalSession(model.config(), columns=100, rows=28, arguments=["--lang", "en-US"]) as terminal:
                terminal.send(b"START\r")
                terminal.wait_for(lambda: model.ready.is_set(), timeout=15)
                terminal.send(b"STREAM-DRAFT")
                terminal.wait_for(lambda: "STREAM-DRAFT" in terminal.text())
                column, row = locate(terminal, "DRAFT")
                click(terminal, column, row)
                terminal.pump(.2)
                terminal.send(b"EDIT-")
                terminal.wait_for(lambda: "STREAM-EDIT-DRAFT" in terminal.text())

    def test_chat_input_click_returns_focus_from_review_without_closing_it(self):
        """无参数；差异区打开时点击左侧输入定位并切回编辑，右侧仍保持显示，返回无。"""
        with TerminalSession(columns=160, rows=32, prepare=prepare_repository,
                             arguments=["--view", "fullscreen", "--lang", "en-US"]) as terminal:
            terminal.wait_for(lambda: "tracked.txt" in terminal.text())
            terminal.send(b"REVIEW-DRAFT")
            terminal.wait_for(lambda: "REVIEW-DRAFT" in terminal.text())
            column, row = locate(terminal, "tracked.txt")
            click(terminal, column, row)
            terminal.wait_for(lambda: "working-line" in terminal.text())
            column, row = locate(terminal, "DRAFT")
            click(terminal, column, row)
            terminal.pump(.2)
            terminal.send(b"EDIT-")
            terminal.wait_for(lambda: "REVIEW-EDIT-DRAFT" in terminal.text())
            self.assertIn("Review", lines(terminal)[0][80:])
            self.assertIn("working-line", terminal.text())


if __name__ == "__main__":
    unittest.main()
