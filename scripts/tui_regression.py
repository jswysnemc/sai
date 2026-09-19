# /// script
# requires-python = ">=3.11"
# dependencies = ["pyte==0.8.2"]
# ///
"""【终端】【回归入口】运行隔离终端测试；使用 uv run scripts/tui_regression.py。"""

from pathlib import Path
import unittest


def main():
    """发现并运行同目录测试；无参数，返回是否全部通过。"""
    suite = unittest.defaultTestLoader.discover(str(Path(__file__).with_suffix("")))
    return unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful()


if __name__ == "__main__":
    raise SystemExit(0 if main() else 1)
