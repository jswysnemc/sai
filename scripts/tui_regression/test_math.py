"""【终端】【公式回归】行内公式不断句，定界符同行的多行公式不报空公式。"""
import unittest
from mock_math_model import MockMathModel
from terminal import TerminalSession


class MathRenderTests(unittest.TestCase):
    """覆盖公式从模型流到终端屏幕的完整链路。"""

    def test_formulas_render_without_breaking_lines_or_errors(self):
        """行内公式后的文字留在同一行，aligned 块不出现渲染错误；返回无。"""
        with MockMathModel() as model, TerminalSession(model.config()) as terminal:
            terminal.send(b"show formulas\r")
            # 块级公式在无图形协议的伪终端里走字符画降级，出图较慢
            terminal.wait_for(lambda: "MATH-DONE" in terminal.text(history=True), timeout=40)
            text = terminal.text(history=True)
            self.assertNotIn("render failed", text)
            self.assertNotIn("content is empty", text)
            line = next(row for row in text.splitlines() if "质能等价" in row)
            self.assertIn("INLINE-TAIL", line, text)


if __name__ == "__main__":
    unittest.main()
