"""【全屏视图】【交互回归】验证固定工作状态和展开段落的正文收起入口。"""

import unittest

from mock_model import MockModel
from terminal import TerminalSession
from test_fullscreen import TurnModel, click, lines


class FullscreenActivityTests(unittest.TestCase):
    """使用本地模型和隔离伪终端验证真实按键、鼠标与尺寸变化。"""

    def test_working_stays_fixed_when_reading_history(self):
        """滚动旧消息和缩窄窗口时，Working 都留在输入框附近；无参数，无返回值。"""
        with MockModel(row_count=45) as model, TerminalSession(
            model.config(), columns=100, rows=30
        ) as terminal:
            terminal.send(b"Show the fixture table\r")
            terminal.wait_for(model.ready.is_set, timeout=15)
            terminal.send(b"\x0f")
            terminal.wait_for(lambda: "1/1" in lines(terminal)[0])
            status_row = next(index for index, line in enumerate(lines(terminal)) if "Working" in line)
            self.assertGreaterEqual(status_row, 20)
            click(terminal, 10, 5, kind=64)
            terminal.pump(.2)
            self.assertIn("Working", lines(terminal)[status_row])
            self.assertFalse(any("Working" in line for line in lines(terminal)[:status_row]))
            terminal.resize(56, 18)
            terminal.wait_for(lambda: any("Working" in line for line in lines(terminal)[8:]))
            self.assertFalse(any("Working" in line for line in lines(terminal)[:8]))
            model.release.set()
            terminal.wait_for(lambda: "Working" not in terminal.text(), timeout=10)

    def test_expanded_reasoning_collapses_from_its_body(self):
        """Ctrl+O 进入全屏后思考默认展开，点击正文收起，点击标题恢复；无参数，无返回值。"""
        with TurnModel() as model, TerminalSession(model.config(), columns=100, rows=32,
                arguments=["--lang", "en-US"]) as terminal:
            terminal.send(b"ALPHA\r")
            terminal.wait_for(lambda: "ANSWER-ALPHA-7" in terminal.text(), timeout=15)
            terminal.send(b"\x0f")
            terminal.wait_for(lambda: "1/1" in lines(terminal)[0])
            self.assertNotIn("Ctrl+O exit", lines(terminal)[0])
            terminal.send(b"\x1b[1;5H")
            terminal.wait_for(lambda: "THINK-ALPHA-4" in terminal.text())
            body_row = next(index for index, line in enumerate(lines(terminal)) if "THINK-ALPHA-4" in line)
            click(terminal, 8, body_row)
            terminal.wait_for(lambda: "THINK-ALPHA-4" not in terminal.text())
            title_row = next(index for index, line in enumerate(lines(terminal)) if "Thought for" in line)
            click(terminal, 2, title_row)
            terminal.wait_for(lambda: "THINK-ALPHA-4" in terminal.text())


if __name__ == "__main__":
    unittest.main()
