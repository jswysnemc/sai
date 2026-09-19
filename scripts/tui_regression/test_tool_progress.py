"""【终端】【行数回归】检查文件工具参数增长时的实时增删统计。"""

import json
import unittest

from mock_tool_model import MockToolModel
from terminal import TerminalSession


class ToolProgressTests(unittest.TestCase):
    """从本地 HTTP 参数流验证实际终端画面中的数字变化。"""

    def assert_progress(self, tool_name, overwrite=False, old_lines=1, counts=(1, 3, 80, 160)):
        """按工具、覆盖模式和原文行数构造参数；counts 为各阶段行数，返回无。"""
        with MockToolModel(tool_name) as model, TerminalSession(model.config()) as terminal:
            path = "fixture.txt"
            old = "\n".join(f"old-{index}-" + "x" * 100 for index in range(old_lines))
            if overwrite or tool_name == "str_replace":
                (terminal.root / path).write_text(old)
            if tool_name == "str_replace":
                prefix = json.dumps({"path": path, "old_string": old})[:-1] + ', "new_string": "'
                removed = old_lines
            else:
                prefix = '{"path":"fixture.txt","content":"'
                removed = 0
            terminal.send(b"Edit the fixture\r")
            terminal.wait_for(model.ready.is_set)
            model.fragments.put(prefix)
            previous = 0
            for count in counts:
                content = "\n".join(f"new-{index}-" + "y" * 100 for index in range(previous, count))
                if previous:
                    content = "\n" + content
                model.fragments.put(json.dumps(content)[1:-1])
                terminal.wait_for(lambda: f"+{count} -{removed}" in terminal.text(), timeout=3)
                self.assertIn(path, terminal.text())
                previous = count

    def test_write_line_count_keeps_growing(self):
        """新增文件随每批参数增长持续更新行数；返回无。"""
        self.assert_progress("write_file")

    def test_overwrite_line_count_keeps_growing(self):
        """覆盖文件时随新内容增长更新行数；返回无。"""
        self.assert_progress("write_file", overwrite=True)

    def test_replace_line_count_keeps_growing(self):
        """替换过程中随新内容增长更新行数；返回无。"""
        self.assert_progress("str_replace")

    def test_replace_after_long_original_shows_line_counts(self):
        """原文较长时仍能统计其后的替换文本；返回无。"""
        self.assert_progress("str_replace", old_lines=1000)

    def test_large_write_line_count_keeps_growing(self):
        """写入参数超过 64K 字符后仍随内容增长更新行数；返回无。"""
        self.assert_progress("write_file", counts=(1, 3, 800, 1000))
