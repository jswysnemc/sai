"""【终端图片】【协议断言】重放终端控制序列，读取 Kitty 图片的实际放置坐标。"""

import re

import pyte


def placements(raw, columns, rows):
    """参数 raw 为原始输出、columns/rows 为屏幕尺寸；返回图片的起始行、列和占位高度列表。"""
    screen = pyte.Screen(columns, rows)
    stream = pyte.Stream(screen)
    output = raw.decode("utf-8", "replace")
    found = []
    cursor = 0
    for match in re.finditer(r"\x1b_G([^\x1b]*)\x1b\\", output):
        stream.feed(output[cursor:match.start()])
        control = dict(field.split("=", 1) for field in match.group(1).split(",") if "=" in field)
        if control.get("a") == "p" and "r" in control:
            found.append((screen.cursor.y, screen.cursor.x, int(control["r"])))
        cursor = match.end()
    return found
