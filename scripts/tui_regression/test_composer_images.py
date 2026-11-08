"""【输入图片】【终端回归】验证真实输入历史恢复、缩放、切屏与删除。"""

import base64
import json
import re
import unittest

from kitty_assertions import placements
from terminal import TerminalSession
from test_activity_images import png_image


def prepare_history(root, environment, binary):
    """【输入图片】【附件样本】参数为隔离目录、环境及程序；写入图片历史，返回无。"""
    path = root / "composer.png"
    png_image(path)
    source = "data:image/png;base64," + base64.b64encode(path.read_bytes()).decode()
    markers = [f"[image {index} 640x480]" for index in range(1, 4)]
    entry = {"text": "".join(markers), "attachments": [
        {"marker": marker, "content": source, "kind": "image"} for marker in markers]}
    history = root / "state/sai/input-history.jsonl"
    history.parent.mkdir(parents=True, exist_ok=True)
    history.write_text(json.dumps(entry) + "\n")


def active_placements(raw):
    """【输入图片】【放置状态】参数为完整 Kitty 输出；返回尚未删除的图片与放置键集合。"""
    active = set()
    for match in re.finditer(rb"\x1b_G([^\x1b]*)\x1b\\", bytes(raw)):
        control = dict(field.split(b"=", 1) for field in match.group(1).split(b";", 1)[0].split(b",") if b"=" in field)
        key = (control.get(b"i"), control.get(b"p"))
        if control.get(b"a") == b"p":
            active.add(key)
        elif control.get(b"a") == b"d":
            if control.get(b"d") == b"a":
                active.clear()
            elif control.get(b"d") == b"i":
                active.discard(key)
    return active


class ComposerImageTests(unittest.TestCase):
    """【输入图片】【回归环境】使用隔离输入历史，不读取系统剪贴板或调用外部模型。"""

    def test_composer_images_restore_resize_switch_and_delete(self):
        """无参数；图片按六列三行展示，打字不重传，窄屏与删除清理旧放置。"""
        with TerminalSession(columns=80, rows=24, prepare=prepare_history,
                             arguments=["--lang", "en-US"], kitty_graphics=True) as terminal:
            terminal.send(b"\x1b[A")
            terminal.wait_for(lambda: len(active_placements(terminal.raw)) == 3)
            initial = placements(bytes(terminal.raw), 80, 24)
            self.assertEqual([column for _, column, _ in initial[-3:]], [3, 10, 17])
            self.assertTrue(all(height == 3 for _, _, height in initial[-3:]))
            self.assertNotIn("iVBOR", terminal.text())
            before = len(terminal.raw)
            terminal.send(b"x")
            terminal.wait_for(lambda: "480]x" in terminal.text())
            self.assertNotIn(b"\x1b_Ga=t", terminal.raw[before:])
            terminal.send(b"\x7f")
            terminal.resize(16, 24)
            terminal.wait_for(lambda: len(active_placements(terminal.raw)) == 1)
            self.assertIn("+2", terminal.text())
            terminal.resize(80, 24)
            terminal.wait_for(lambda: len(active_placements(terminal.raw)) == 3)
            before = len(terminal.raw)
            terminal.send(b"\x0f")
            terminal.wait_for(lambda: b"\x1b_Ga=p" in terminal.raw[before:])
            terminal.pump(.3)
            self.assertIn("[image 1 640x480]", terminal.text())
            before = len(terminal.raw)
            terminal.send(b"\x0c")
            terminal.wait_for(lambda: b"\x1b_Ga=p" in terminal.raw[before:])
            terminal.send(b"\x7f")
            terminal.wait_for(lambda: len(active_placements(terminal.raw)) == 2)
            terminal.send(b"\x7f\x7f")
            terminal.wait_for(lambda: not active_placements(terminal.raw))
            self.assertNotIn("[image 1", terminal.text())

    def test_composer_images_plain_terminal_keeps_markers_only(self):
        """无参数；无图形协议时只展示原始附件标签，不输出图片控制序列。"""
        with TerminalSession(columns=80, rows=24, prepare=prepare_history,
                             arguments=["--lang", "en-US"]) as terminal:
            terminal.send(b"\x1b[A")
            terminal.wait_for(lambda: "[image 3 640x480]" in terminal.text())
            self.assertNotIn(b"\x1b_G", terminal.raw)
            self.assertNotIn(b"1337;File", terminal.raw)


if __name__ == "__main__":
    unittest.main()
