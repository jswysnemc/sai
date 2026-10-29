"""【终端】【紧凑正文回归】检查真实终端的思考窗口、问答内容和选择标记。"""

import unittest

from mock_activity_model import ActivityModel
from terminal import TerminalSession
from test_fullscreen import lines


class ActivityCompactContentTests(unittest.TestCase):
    """【终端】【紧凑正文】仅使用隔离会话和本地模型响应。"""

    def row_of(self, terminal, text):
        """参数 terminal 为终端、text 为标题；返回首个可见行号。"""
        return next(row for row, line in enumerate(lines(terminal)) if text in line)

    def test_reasoning_has_five_gutter_rows_and_scrolls_in_place(self):
        """无参数；滚轮只改变五行正文，思考标题和输入框保持原位，返回无。"""
        for columns, rows in [(60, 24), (100, 40)]:
            with self.subTest(columns=columns, rows=rows), ActivityModel() as model:
                model.reasoning = "\n".join(f"REASON-{index:02}" for index in range(20))
                with TerminalSession(model.config(), columns=columns, rows=rows,
                                     arguments=["--lang", "en-US"],
                                     ready=lambda t: "Enter a message" in t.text()) as terminal:
                    terminal.send(b"Inspect reasoning window\r")
                    terminal.wait_for(lambda: "REASON-19" in terminal.text())
                    title = self.row_of(terminal, "Thinking")
                    composer = self.row_of(terminal, "→")
                    body = [line for line in lines(terminal) if "REASON-" in line]
                    self.assertEqual(len(body), 5, terminal.text())
                    self.assertTrue(all("│" in line for line in body), body)
                    terminal.send(f"\x1b[<64;8;{title + 3}M".encode())
                    terminal.wait_for(lambda: "REASON-12" in terminal.text())
                    self.assertNotIn("REASON-19", terminal.text())
                    self.assertEqual(self.row_of(terminal, "Thinking"), title)
                    self.assertEqual(self.row_of(terminal, "→"), composer)
                    terminal.pump(.3)
                    self.assertIn("REASON-12", terminal.text())
                    terminal.send(f"\x1b[<65;8;{title + 3}M".encode())
                    terminal.wait_for(lambda: "REASON-19" in terminal.text())
                    self.assertEqual(self.row_of(terminal, "Thinking"), title)

    def test_question_uses_round_choices_and_no_protocol_dump(self):
        """无参数；多选等待时显示全部选项，完成记录只保留问题与已选答案，返回无。"""
        arguments = {"questions": [{"header": "Report scope", "question": "Choose report sections",
            "multiple": True, "options": [
                {"label": "Call matrix", "description": "Matrix explanation"},
                {"label": "Failure cases", "description": "Failure explanation"},
                {"label": "Suggestions", "description": "Suggestion explanation"}]}]}
        with ActivityModel("ask_question", arguments) as model:
            model.advance.set()
            with TerminalSession(model.config(), columns=100, rows=40,
                                 arguments=["--lang", "en-US"]) as terminal:
                terminal.send(b"Inspect question card\r")
                terminal.wait_for(lambda: "Space toggle" in terminal.text())
                self.assertIn("○", terminal.text())
                self.assertNotIn("□", terminal.text())
                self.assertIn("Suggestions", terminal.text())
                terminal.send(b"12")
                terminal.wait_for(lambda: sum("●" in line for line in lines(terminal)
                                             if "Call matrix" in line or "Failure cases" in line) == 2)
                terminal.send(b"\r")
                terminal.wait_for(lambda: "Enter submit" in terminal.text())
                terminal.send(b"\r")
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text())
                rendered = terminal.text()
                for text in ["Choose report sections", "Call matrix", "Failure cases"]:
                    self.assertEqual(rendered.count(text), 1, rendered)
                for text in ["Header:", "Multiple:", "Description:", "Label:", "Input:", "Output:",
                             "Matrix explanation", "Failure explanation", "Suggestions", "○", "□", "■"]:
                    self.assertNotIn(text, rendered)


if __name__ == "__main__":
    unittest.main()
