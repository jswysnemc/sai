# /// script
# requires-python = ">=3.11"
# dependencies = ["pyte>=0.8.2,<0.9"]
# ///
"""通过隔离配置、公共插件命令及真实伪终端验收 Lua 底栏。"""

import argparse
import codecs
import fcntl
import json
import os
import pty
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time
from pathlib import Path

import pyte

from fixture import Fixture


class Terminal:
    """保存真实 TUI 进程及屏幕解析器，所有状态位于临时目录。"""

    def __init__(self, fixture):
        """【底栏验收】【终端启动】接收隔离环境，启动 120 列 TUI，无返回值。"""
        self.master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 120, 0, 0))
        self.screen = pyte.Screen(120, 30)
        self.screen.write_process_input = self.send
        self.stream = pyte.Stream(self.screen)
        self.decoder = codecs.getincrementaldecoder("utf-8")("replace")
        environment = dict(fixture.environment, TERM="xterm-256color", COLORTERM="truecolor")
        self.process = subprocess.Popen(
            [str(fixture.binary), "--lang", "en-US", "--yolo"],
            cwd=fixture.project, env=environment,
            stdin=slave, stdout=slave, stderr=slave, start_new_session=True,
        )
        os.close(slave)
        self.raw = bytearray()

    def send(self, text):
        """【底栏验收】【按键输入】接收终端文本或查询回应，写入主 PTY，无返回值。"""
        os.write(self.master, text.encode())

    def read(self, timeout=0.1):
        """【底栏验收】【屏幕读取】接收等待秒数，解析一次可用输出，无返回值。"""
        if select.select([self.master], [], [], timeout)[0]:
            try:
                data = os.read(self.master, 65536)
            except OSError:
                data = b""
            self.raw.extend(data)
            self.stream.feed(self.decoder.decode(data))
        if self.process.poll() is not None:
            raise AssertionError(f"TUI exited: {self.process.returncode}\n{self.display()}")

    def display(self):
        """【底栏验收】【显示快照】无参数，返回当前可见屏幕文本。"""
        return "\n".join(self.screen.display)

    def wait(self, expected, timeout=12):
        """【底栏验收】【可见断言】接收预期文本与超时，返回包含该文本的屏幕。"""
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            self.read()
            if expected in self.display():
                return self.display()
        raise AssertionError(f"Missing visible text: {expected}\n{self.display()}")

    def resize(self, columns):
        """【底栏验收】【窗口缩放】接收列数，调整 PTY 和屏幕尺寸并通知进程，无返回值。"""
        self.screen.resize(lines=30, columns=columns)
        fcntl.ioctl(self.master, termios.TIOCSWINSZ, struct.pack("HHHH", 30, columns, 0, 0))
        os.kill(self.process.pid, signal.SIGWINCH)

    def close(self):
        """【底栏验收】【进程回收】无参数，只结束本脚本创建的隔离进程并关闭 PTY。"""
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=3)
        os.close(self.master)


def verify(fixture, example):
    """【底栏验收】【完整链路】接收隔离环境及外部包目录，返回真实终端验收记录。"""
    inspection = fixture.plugins("check", example)
    assert inspection["events"] == ["tui_status"]
    assert inspection["tools"] == []
    fixture.plugins("install", example)
    fixture.plugins("configure", "tui-status", example / "settings.example.json")
    fixture.plugins("enable", "tui-status", "--allow-tui-status")
    info = fixture.plugins("info", "tui-status")["plugins"][0]
    assert info["effective_grants"]["tui_status"] is True
    config_dir = fixture.root / "config/sai"
    (config_dir / "config.jsonc").write_text(json.dumps({
        "active_provider": "fixture",
        "providers": [{"id": "fixture", "display_name": "Fixture", "base_url": "http://127.0.0.1:1/v1",
                       "default_model": "fixture-model", "thinking_level": "high", "api_key": "fixture"}],
        "tools": {"enabled": False}, "skills": {"enabled": False},
        "memory": {"enabled": False}, "load_instruction_files": False,
    }), encoding="utf-8")
    terminal = Terminal(fixture)
    screens = {}
    try:
        screens["default"] = terminal.wait("yolo · fixture-model ·")
        assert "project" in screens["default"]
        terminal.send("draft-kept")
        screens["input"] = terminal.wait("draft-kept")
        terminal.resize(50)
        terminal.wait("draft-kept")
        screens["narrow"] = terminal.wait("yolo · fixture-model ·")
        assert len(terminal.screen.display[-1]) <= 50
        terminal.resize(120)
        terminal.send("\x05" + "\x7f" * len("draft-kept"))
        settings = fixture.root / "custom-status.json"
        settings.write_text(json.dumps({"left":["label","model"],"right":["thinking"],
                                       "label":"Lua verified","separator":" / "}), encoding="utf-8")
        fixture.plugins("configure", "tui-status", settings)
        terminal.send("/plugins reload\r")
        terminal.wait("Plugins reloaded.")
        screens["configured"] = terminal.wait("Lua verified / fixture-model")
        assert "Connection interrupted" not in screens["configured"]
        fixture.plugins("disable", "tui-status")
        terminal.send("/plugins reload\r")
        deadline = time.monotonic() + 12
        while time.monotonic() < deadline:
            terminal.read()
            footer = next((line for line in reversed(terminal.screen.display) if line.strip()), "")
            if "fixture-model" in footer and "Lua verified" not in footer and " / " not in footer:
                screens["disabled"] = terminal.display()
                break
        assert "disabled" in screens, terminal.display()
        return screens
    finally:
        (fixture.root / "terminal.raw").write_bytes(terminal.raw)
        terminal.close()


def main():
    """【底栏验收】【脚本入口】读取命令行路径，创建隔离环境并输出 JSON 验收证据，无返回值。"""
    if sys.platform != "linux":
        raise SystemExit("This isolated XDG/PTY smoke test requires Linux")
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=Path("target/debug/sai"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    example = Path(__file__).resolve().parents[2] / "examples/lua-plugins/tui-status"
    with tempfile.TemporaryDirectory(prefix="sai-tui-status-") as temporary:
        fixture = Fixture(args.binary.resolve(), Path(temporary))
        screens = verify(fixture, example)
        args.output.write_text(json.dumps({"ok": True, "screens": screens, "calls": fixture.calls},
                                         ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"ok": True, "checks": list(screens), "output": str(args.output)}))


if __name__ == "__main__":
    main()
