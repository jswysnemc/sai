"""【Git预览】【真实终端】文件组件、比较控件与键盘焦点的端到端回归。"""

import subprocess
import unittest

from terminal import TerminalSession
from test_fullscreen import TurnModel, click, lines


def prepare_repository(root, environment, binary):
    """参数为隔离目录、环境和程序；准备暂存、工作树和新增文件，返回无。"""
    for args in [["init"], ["config", "user.name", "Fixture"],
                 ["config", "user.email", "fixture@example.invalid"]]:
        subprocess.run(["git", *args], cwd=root, check=True, capture_output=True)
    (root / ".gitignore").write_text("config/\ndata/\nstate/\ncache/\npictures/\n")
    (root / "tracked.txt").write_text("original-line\n")
    subprocess.run(["git", "add", "."], cwd=root, check=True, capture_output=True)
    subprocess.run(["git", "commit", "-m", "test: diff preview fixture"],
                   cwd=root, check=True, capture_output=True)
    (root / "tracked.txt").write_text("staged-line\n")
    subprocess.run(["git", "add", "tracked.txt"], cwd=root, check=True, capture_output=True)
    (root / "tracked.txt").write_text("working-line\n" + "".join(f"CODE-ROW-{index}\n" for index in range(35)))
    (root / "new.txt").write_text("new-file-content\n")


class GitPreviewTests(unittest.TestCase):
    """使用真实鼠标与键盘事件验证文件浏览和差异窗口。"""

    def test_file_click_modes_scroll_resize_and_draft_isolation(self):
        """无参数；固定左右分栏中，文件、差异、聊天分别滚动，标题不动且草稿保留，返回无。"""
        def prepare(root, environment, binary):
            """参数为隔离目录、环境和程序；增加文件以覆盖独立文件列表滚动，返回无。"""
            prepare_repository(root, environment, binary)
            for index in range(25):
                (root / f"extra-{index:02}.rs").write_text(f"extra line {index}\n")

        with TurnModel() as model:
            config = model.config()
            config["display"]["fullscreen"] = True
            with TerminalSession(config, columns=160, rows=32, prepare=prepare,
                                 arguments=["--lang", "en-US"]) as terminal:
                terminal.wait_for(lambda: "tracked.txt" in terminal.text())
                for message in [b"ALPHA", b"BRAVO"]:
                    terminal.send(message + b"\r")
                    terminal.wait_for(lambda: f"ANSWER-{message.decode()}-7" in terminal.text(), timeout=15)
                terminal.send(b"KEPT-DRAFT")
                terminal.wait_for(lambda: "KEPT-DRAFT" in terminal.text())
                before = subprocess.check_output(["git", "status", "--porcelain"], cwd=terminal.root)
                row = next(index for index, line in enumerate(lines(terminal)) if "tracked.txt" in line[120:])
                click(terminal, 156, row)
                terminal.wait_for(lambda: "working-line" in terminal.text())
                # 1. 【终端审查】【固定左右】左侧背景保持原样，右侧红绿背景只出现在增删代码行
                self.assertIn("Review", lines(terminal)[1][80:])
                self.assertTrue(any("ANSWER-BRAVO" in line[:80] for line in lines(terminal)))
                self.assertFalse(any("CODE-ROW" in line[:80] for line in lines(terminal)))
                self.assertTrue(all(terminal.screen.buffer[row][column].bg == "default"
                                    for row in range(9) for column in range(80, 160)))
                added_row = next(index for index, line in enumerate(lines(terminal)) if "working-line" in line[80:])
                removed_row = next(index for index, line in enumerate(lines(terminal)) if "original-line" in line[80:])
                self.assertEqual(terminal.screen.buffer[added_row][90].bg, "142e1e")
                self.assertEqual(terminal.screen.buffer[removed_row][90].bg, "3f191e")
                self.assertTrue(all(terminal.screen.buffer[added_row][column].bg == "142e1e"
                                    for column in range(82, 159)))
                self.assertEqual(terminal.screen.buffer[added_row][159].bg, "default")
                self.assertNotIn("Original line", terminal.text())
                self.assertIn("@@ -", terminal.text())
                self.assertNotEqual(terminal.screen.buffer[0][5].bg, "default")
                draft_row = next(index for index, line in enumerate(lines(terminal)) if "KEPT-DRAFT" in line[:80])
                self.assertEqual(terminal.screen.buffer[draft_row][5].bg, terminal.screen.buffer[0][5].bg)
                self.assertIn("+", lines(terminal)[4][80:])
                self.assertIn("-", lines(terminal)[4][80:])
                header = lines(terminal)[4]
                self.assertNotEqual(terminal.screen.buffer[4][header.index("+", 80)].fg,
                                    terminal.screen.buffer[4][header.index("-", 80)].fg)
                # 2. 【终端审查】【比较控件】模式只在悬停时反色，切换读取不同内容
                column = lines(terminal)[3].index("[ Staged ]") + 2
                terminal.send(f"\x1b[<35;{column + 1};4M".encode())
                terminal.wait_for(lambda: terminal.screen.buffer[3][column].reverse)
                self.assertFalse(terminal.screen.buffer[3][column].underscore)
                terminal.send(b"\x1b[<35;6;6M")
                terminal.wait_for(lambda: not terminal.screen.buffer[3][column].reverse)
                click(terminal, column, 3)
                terminal.wait_for(lambda: "staged-line" in terminal.text() and "working-line" not in terminal.text())
                column = lines(terminal)[3].index("[ Unstaged ]") + 2
                click(terminal, column, 3)
                terminal.wait_for(lambda: "staged-line" in terminal.text() and "working-line" in terminal.text())
                diff_row = next(index for index, line in enumerate(lines(terminal)) if line[80:].lstrip("│ ").startswith("tracked.txt"))
                sticky_header = lines(terminal)[diff_row][80:]
                chat_before = [line[:80] for line in lines(terminal)]
                diff_before = [line[80:] for line in lines(terminal)[diff_row:]]
                files_before = [line[80:] for line in lines(terminal)[5:diff_row - 1]]
                # 3. 【终端审查】【文件局部滚动】只改变文件清单，不改变聊天、差异或标题
                click(terminal, 110, 5, kind=65)
                terminal.wait_for(lambda: [line[80:] for line in lines(terminal)[5:diff_row - 1]] != files_before)
                self.assertEqual([line[:80] for line in lines(terminal)], chat_before)
                self.assertEqual([line[80:] for line in lines(terminal)[diff_row:]], diff_before)
                files_before = [line[80:] for line in lines(terminal)[5:diff_row - 1]]
                click(terminal, 110, diff_row + 2, kind=65)
                terminal.wait_for(lambda: [line[80:] for line in lines(terminal)[diff_row:]] != diff_before)
                self.assertEqual([line[:80] for line in lines(terminal)], chat_before)
                self.assertEqual([line[80:] for line in lines(terminal)[5:diff_row - 1]], files_before)
                self.assertEqual(lines(terminal)[diff_row][80:], sticky_header)
                terminal.send(b"\x1b[6~")
                terminal.wait_for(lambda: "CODE-ROW-20" in terminal.text())
                self.assertEqual(lines(terminal)[diff_row][80:], sticky_header)
                right_before = [line[80:] for line in lines(terminal)]
                # 4. 【终端审查】【聊天独立】聊天滚动不改变审查区，也不会关闭审查
                click(terminal, 10, 6, kind=64)
                terminal.wait_for(lambda: [line[:80] for line in lines(terminal)] != chat_before)
                self.assertEqual([line[80:] for line in lines(terminal)], right_before)
                terminal.send(b"\x1b[17~")
                terminal.wait_for(lambda: "F6 chat" in terminal.text())
                terminal.send(b"NEXT")
                terminal.wait_for(lambda: "KEPT-DRAFTNEXT" in terminal.text())
                self.assertIn("Review", lines(terminal)[1][80:])
                # 5. 【终端审查】【缩放保持分栏】窄屏仍然左右分栏，不回退成浮动窗口
                terminal.resize(100, 26)
                terminal.wait_for(lambda: "Review" in lines(terminal)[1][50:])
                self.assertFalse(any("CODE-ROW" in line[:50] for line in lines(terminal)))
                self.assertIn("KEPT-DRAFTNEXT", terminal.text())
                click(terminal, 97, 1)
                terminal.wait_for(lambda: "Review" not in lines(terminal)[1])
                self.assertEqual(subprocess.check_output(["git", "status", "--porcelain"], cwd=terminal.root), before)

    def test_keyboard_focus_opens_new_file_and_returns_to_composer(self):
        """无参数；F6 聚焦、方向键选择、Enter 打开新增文件，Esc 返回输入，返回无。"""
        with TurnModel() as model:
            config = model.config()
            config["display"]["fullscreen"] = True
            with TerminalSession(config, columns=160, rows=32, prepare=prepare_repository,
                                 arguments=["--lang", "en-US"]) as terminal:
                terminal.wait_for(lambda: "new.txt" in terminal.text())
                terminal.send(b"DRAFT\x1b[17~")
                terminal.wait_for(lambda: "Click to collapse" in terminal.text())
                for _ in range(4):
                    terminal.send(b"\x1b[B")
                terminal.send(b"\r")
                terminal.wait_for(lambda: "new-file-content" in terminal.text())
                terminal.send(b"\x1b")
                terminal.wait_for(lambda: "[ Working tree ]" not in terminal.text())
                terminal.send(b"\x1b")
                terminal.wait_for(lambda: "F6 focus" in terminal.text())
                terminal.send(b"-KEPT")
                terminal.wait_for(lambda: "DRAFT-KEPT" in terminal.text())

    def test_syntax_highlight_sticky_filename_and_narrow_layout(self):
        """无参数；真实终端保留语法颜色和增删标记，缩放后文件名仍固定，返回无。"""
        def prepare(root, environment, binary):
            """参数为隔离目录、环境和程序；创建长路径源码差异，返回无。"""
            prepare_repository(root, environment, binary)
            source = root / "src/long-directory/deeper-module/syntax.rs"
            source.parent.mkdir(parents=True)
            source.write_text("fn value() -> usize {\n    1\n}\n")
            subprocess.run(["git", "add", "src"], cwd=root, check=True, capture_output=True)
            subprocess.run(["git", "commit", "-m", "test: syntax fixture"],
                           cwd=root, check=True, capture_output=True)
            source.write_text('fn value() -> usize {\n    let label = "sample";\n    let count = 42;\n'
                              + "".join(f"    let row_{index} = count + {index};\n" for index in range(40))
                              + "    count\n}\n")

        with TurnModel() as model:
            config = model.config()
            config["display"]["fullscreen"] = True
            with TerminalSession(config, columns=160, rows=32, prepare=prepare,
                                 arguments=["--lang", "en-US"]) as terminal:
                terminal.wait_for(lambda: "syntax.rs" in terminal.text())
                row = next(index for index, line in enumerate(lines(terminal)) if "syntax.rs" in line[120:])
                click(terminal, 156, row)
                terminal.wait_for(lambda: 'let label = "sample"' in terminal.text())
                row = next(index for index, line in enumerate(lines(terminal)) if "let label" in line[80:])
                text = lines(terminal)[row]
                keyword = text.index("let", 80)
                string = text.index("sample", 80)
                marker = text.index("+", 80)
                self.assertNotEqual(terminal.screen.buffer[row][keyword].fg, terminal.screen.buffer[row][string].fg)
                self.assertNotEqual(terminal.screen.buffer[row][marker].fg, terminal.screen.buffer[row][keyword].fg)
                self.assertTrue(all(terminal.screen.buffer[row][column].bg == "142e1e" for column in range(82, 159)))
                self.assertEqual(terminal.screen.buffer[row][80].bg, "default")
                self.assertEqual(terminal.screen.buffer[row][159].bg, "default")
                header_row = next(index for index, line in enumerate(lines(terminal))
                                  if line[80:].lstrip("│ ").startswith("src/long-directory"))
                sticky = lines(terminal)[header_row][80:]
                terminal.send(b"\x1b[6~")
                terminal.wait_for(lambda: "let row_20" in terminal.text())
                self.assertEqual(lines(terminal)[header_row][80:], sticky)
                # 1. 【终端审查】【横向着色】语法词元复位后，整行背景也必须完整保留
                terminal.send(b"\x1b[C")
                terminal.pump(.2)
                row = next(index for index, line in enumerate(lines(terminal)) if "row_20" in line[80:])
                self.assertTrue(all(terminal.screen.buffer[row][column].bg == "142e1e" for column in range(82, 159)))
                self.assertEqual(lines(terminal)[header_row][80:], sticky)
                terminal.resize(80, 24)
                terminal.wait_for(lambda: "Review" in lines(terminal)[1][40:])
                self.assertIn("[ Working ] [ Staged ] [ Unstaged ]", lines(terminal)[3][40:])
                self.assertTrue(any("syntax.rs" in line[40:] for line in lines(terminal)))
                self.assertFalse(any("row_" in line[:40] for line in lines(terminal)))
                click(terminal, 77, 1)
                terminal.wait_for(lambda: "Review" not in lines(terminal)[1])


if __name__ == "__main__":
    unittest.main()
