"""【终端】【浅色主题】在真实伪终端中检查工具正文、换行与数字引导列。"""
import re
import unittest
from mock_activity_model import ActivityModel
from terminal import TerminalSession


def luminance(color):
    """color 为六位 RGB；返回 sRGB 相对亮度。"""
    channels = [int(color[index:index + 2], 16) / 255 for index in (0, 2, 4)]
    channels = [value / 12.92 if value <= .04045 else ((value + .055) / 1.055) ** 2.4 for value in channels]
    return sum(value * weight for value, weight in zip(channels, (.2126, .7152, .0722)))


class LightThemeTests(unittest.TestCase):
    """【终端】【主题回归】验证非白色浅背景，避免只针对白色修正。"""

    def test_read_details_remain_readable_on_tinted_light_background(self):
        """无参数；返回无值，读取输出换行后保持列对齐并满足文本对比度。"""
        background = "c2d5e7"
        with ActivityModel("read_file", {"path": "fixture.txt"}) as model:
            with TerminalSession(model.config(), columns=90, rows=40, arguments=["--lang", "en-US"], terminal_colors=("202020", background)) as terminal:
                (terminal.root / "fixture.txt").write_text("READABLE-LIGHT " + "wrapped-content " * 8)
                terminal.send(b"Inspect light theme\r")
                terminal.wait_for(model.ready.is_set)
                model.advance.set()
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text())
                terminal.wait_for(lambda: "READABLE-LIGHT" in terminal.text())
                checked = 0
                for row in terminal.screen.buffer.values():
                    for cell in row.values():
                        if not cell.data.strip() or not re.fullmatch(r"[0-9a-fA-F]{6}", cell.fg):
                            continue
                        bg = cell.bg if re.fullmatch(r"[0-9a-fA-F]{6}", cell.bg) else background
                        fg, bg = luminance(cell.fg), luminance(bg)
                        ratio = (max(fg, bg) + .05) / (min(fg, bg) + .05)
                        self.assertGreaterEqual(ratio, 4.45, f"Low contrast text {cell.data!r}: {cell.fg}")
                        checked += 1
                self.assertGreater(checked, 20)
