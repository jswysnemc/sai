# PTY 冒烟测试：在隔离环境中驱动 sai REPL，验证 transcript 渲染不吞内容
# 运行方式: uv run --with pyte python scripts/tui_smoke.py （需先 cargo build）
import fcntl
import os
import pty
import select
import signal
import struct
import sys
import termios
import time

import pyte

BIN = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "target/debug/sai",
)
HOME = "/tmp/sai-smoke-home"
COLS, ROWS = 100, 30


def spawn(argv):
    """启动指定命令并返回 (pid, master_fd)"""
    os.makedirs(HOME, exist_ok=True)
    env = dict(os.environ)
    env.update(
        {
            "HOME": HOME,
            "XDG_CONFIG_HOME": f"{HOME}/.config",
            "XDG_DATA_HOME": f"{HOME}/.local/share",
            "XDG_STATE_HOME": f"{HOME}/.local/state",
            "XDG_CACHE_HOME": f"{HOME}/.cache",
            "TERM": "xterm-256color",
            "LANG": "zh_CN.UTF-8",
        }
    )
    pid, fd = pty.fork()
    if pid == 0:
        os.execve(argv[0], argv, env)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
    return pid, fd


class Session:
    """一个 REPL PTY 会话与其虚拟屏幕"""

    def __init__(self, argv):
        self.pid, self.fd = spawn(argv)
        self.screen = pyte.HistoryScreen(COLS, ROWS, history=500)
        self.stream = pyte.Stream(self.screen)

    def pump(self, seconds):
        deadline = time.time() + seconds
        while time.time() < deadline:
            ready, _, _ = select.select([self.fd], [], [], 0.1)
            if not ready:
                continue
            try:
                data = os.read(self.fd, 65536)
            except OSError:
                break
            if not data:
                break
            self.stream.feed(data.decode("utf-8", "replace"))
            # 真实终端会响应 DSR 光标查询，虚拟屏需要手动应答
            if b"\x1b[6n" in data:
                row = self.screen.cursor.y + 1
                col = self.screen.cursor.x + 1
                os.write(self.fd, f"\x1b[{row};{col}R".encode())

    def text(self):
        return "\n".join(row.rstrip() for row in self.screen.display)

    def scrollback(self):
        lines = []
        for line in self.screen.history.top:
            cells = sorted(line.items())
            lines.append("".join(cell.data for _, cell in cells))
        return "\n".join(lines)

    def send(self, data):
        os.write(self.fd, data)

    def close(self):
        try:
            os.kill(self.pid, signal.SIGKILL)
        except OSError:
            pass
        os.close(self.fd)

    def boot(self):
        self.pump(3.0)
        text = self.text()
        # 首次启动向导：拒绝 shell 集成后进入 REPL
        if "是否集成" in text or "integrate" in text.lower():
            self.send(b"n\r")
            self.pump(3.0)
            text = self.text()
        assert "Sai" in text or "sai" in text, f"欢迎界面缺失:\n{text}"


def scenario_basic():
    """基础链路：shell 命令、长输出滚动、resize、scrollback 保留"""
    session = Session([BIN])
    try:
        session.boot()
        session.send(b"!echo smoke-shell-ok\r")
        session.pump(2.0)
        assert "smoke-shell-ok" in session.text(), f"shell 输出缺失:\n{session.text()}"

        session.send(b"/help\r")
        session.pump(2.0)

        fcntl.ioctl(session.fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, 80, 0, 0))
        os.kill(session.pid, signal.SIGWINCH)
        session.pump(1.5)

        session.send(b"!echo after-resize-ok\r")
        session.pump(2.0)
        final = session.text()
        assert "after-resize-ok" in final, f"resize 后输出缺失:\n{final}"
        combined = final + session.scrollback()
        assert "smoke-shell-ok" in combined, "早期输出被吞掉"
        print("scenario_basic: PASS")
    finally:
        session.close()


def scenario_bottom_start():
    """屏幕底部启动：welcome 面板必须完整显示，上方 shell 输出保留"""
    session = Session(["/bin/sh", "-c", f"seq 1 40; exec {BIN}"])
    try:
        session.boot()
        text = session.text()
        assert "╭" in text and "╰" in text, f"welcome 面板不完整:\n{text}"
        assert "permissions" in text or "权限" in text, f"welcome 字段缺失:\n{text}"
        # 贴底时 footer 不能被下方清理逻辑误清
        assert "auto" in text, f"底栏缺失:\n{text}"
        combined = text + session.scrollback()
        assert "39" in combined and "40" in combined, "启动前的 shell 输出丢失"
        print("scenario_bottom_start: PASS")
    finally:
        session.close()


def scenario_slash_panel_cleanup():
    """slash 面板收起后下方不残留旧建议行"""
    session = Session([BIN])
    try:
        session.boot()
        session.send(b"/")
        session.pump(1.5)
        with_panel = session.text()
        assert "创建新会话" in with_panel, f"slash 面板未出现:\n{with_panel}"

        session.send(b"\x7f")  # Backspace 删除 "/"
        session.pump(1.5)
        after = session.text()
        assert "创建新会话" not in after, f"slash 面板残留:\n{after}"
        # 底栏应恢复
        assert "auto" in after or "%" in after, f"底栏未恢复:\n{after}"
        print("scenario_slash_panel_cleanup: PASS")
    finally:
        session.close()


def main():
    scenario_basic()
    scenario_bottom_start()
    scenario_slash_panel_cleanup()
    print("---- 结果: ALL PASS ----")


if __name__ == "__main__":
    sys.exit(main())
