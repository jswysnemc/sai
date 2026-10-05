"""【终端】【测试环境】隔离配置、进程和终端历史，记录真实控制序列。"""

import codecs
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import signal
import struct
import tempfile
import termios
import time

import pyte


class TerminalSession:
    """提供按键输入、尺寸变化和完整屏幕断言所需的伪终端。"""

    def __init__(self, config=None, columns=80, rows=24, prepare=None, arguments=None, workspace=None, ready=None, kitty_keyboard=False):
        """config 为配置，columns/rows 为尺寸；prepare 初始化数据，arguments/workspace 指定启动参数与目录，ready 指定就绪条件；kitty_keyboard 为真时应答键盘增强协议查询。"""
        self.kitty_keyboard = kitty_keyboard
        self.directory = tempfile.TemporaryDirectory(prefix="sai-terminal-test-")
        self.root = Path(self.directory.name)
        self.ready = ready
        self.raw = bytearray()
        self.screen = pyte.HistoryScreen(columns, rows, history=20000)
        self.stream = pyte.Stream(self.screen)
        self.decoder = codecs.getincrementaldecoder("utf-8")("replace")
        environment = dict(os.environ)
        for key, folder in [("XDG_CONFIG_HOME", "config"), ("XDG_DATA_HOME", "data"),
                            ("XDG_STATE_HOME", "state"), ("XDG_CACHE_HOME", "cache"),
                            ("XDG_PICTURES_DIR", "pictures")]:
            environment[key] = str(self.root / folder)
        environment.update(TERM="xterm-256color", LANG="en_US.UTF-8", LC_ALL="C.UTF-8")
        for key in ["KITTY_WINDOW_ID", "WEZTERM_PANE", "TERM_PROGRAM"]:
            environment.pop(key, None)
        config = config or {"active_provider": "test", "providers": [{
            "id": "test", "display_name": "Test", "base_url": "http://127.0.0.1:1/v1",
            "api_key": "local-fixture", "models": ["test"], "default_model": "test"}]}
        # 产品默认以全局渲染启动并把公式渲染成图片；回归用例按普通模式启动、Ctrl+O 进入全屏，
        # 且断言公式文本，故未显式指定时固定为普通模式与文本公式
        display = config.setdefault("display", {})
        display.setdefault("fullscreen", False)
        display.setdefault("math_images", False)
        target = self.root / "config/sai/config.jsonc"
        target.parent.mkdir(parents=True)
        target.write_text(json.dumps(config))
        binary = Path(os.environ.get("SAI_TEST_BINARY", Path(__file__).resolve().parents[2] / "target/debug/sai")).resolve()
        if not binary.is_file():
            raise FileNotFoundError(f"Build sai first: {binary}")
        self.environment = environment
        self.binary = binary
        if prepare is not None:
            prepare(self.root, environment, binary)
        start_directory = self.root / workspace if workspace else self.root
        start_directory.mkdir(parents=True, exist_ok=True)
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            os.chdir(start_directory)
            os.execve(str(binary), [str(binary)] + list(arguments or []), environment)
        self.resize(columns, rows)

    def __enter__(self):
        """等待输入提示就绪；返回会话本身。"""
        try:
            self.wait_for(lambda: self.ready(self) if self.ready else "auto" in self.text().lower(), timeout=15)
        except Exception as error:
            self.__exit__(type(error), error, error.__traceback__)
            raise
        return self

    def __exit__(self, exc_type, exc, traceback):
        """关闭测试进程；失败时保留控制序列和屏幕，参数为异常信息。"""
        if exc_type:
            artifact = Path(tempfile.mkdtemp(prefix="sai-terminal-failure-"))
            (artifact / "terminal.ansi").write_bytes(self.raw)
            (artifact / "screen.txt").write_text(self.text())
            (artifact / "history.txt").write_text(self.text(history=True))
            print(f"Terminal failure evidence: {artifact}")
        try:
            os.kill(self.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            os.close(self.fd)
        except OSError:
            # 测试已经主动关闭了主端（模拟关闭终端窗口）
            pass
        try:
            os.waitpid(self.pid, 0)
        except ChildProcessError:
            # 测试已经主动回收了进程（例如检查信号退出码）
            pass
        self.directory.cleanup()

    def pump(self, seconds=0.1):
        """读取并解析输出，seconds 为最长等待时间；返回无。"""
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            ready, _, _ = select.select([self.fd], [], [], min(.03, max(0, deadline - time.monotonic())))
            if not ready:
                continue
            try:
                data = os.read(self.fd, 65536)
            except OSError:
                return
            if not data:
                return
            self.raw.extend(data)
            self.stream.feed(self.decoder.decode(data))
            if b"\x1b[6n" in data:
                self.send(f"\x1b[{self.screen.cursor.y + 1};{self.screen.cursor.x + 1}R".encode())
            if b"\x1b]11;?" in data:
                self.send(b"\x1b]11;rgb:0000/0000/0000\x1b\\")
            if self.kitty_keyboard and b"\x1b[?u" in data:
                # 应答键盘增强协议查询与主设备属性，让 sai 真正启用协议
                self.send(b"\x1b[?0u\x1b[?62;22c")

    def wait_for(self, predicate, timeout=10):
        """等待 predicate 成立，timeout 为秒数；超时包含屏幕证据。"""
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            self.pump()
            if predicate():
                return
        raise AssertionError("Terminal condition timed out:\n" + self.text())

    def send(self, data):
        """发送字节序列 data；返回无。"""
        os.write(self.fd, data)

    def resize(self, columns, rows):
        """更新模拟器和真实伪终端尺寸，参数为列数与行数；返回无。"""
        self.screen.resize(rows, columns)
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))
        os.kill(self.pid, signal.SIGWINCH)

    def text(self, history=False):
        """返回可见文本；history 为真时包含原生滚动历史。"""
        lines = []
        if history:
            lines.extend("".join(cell.data for _, cell in sorted(line.items())).rstrip()
                         for line in self.screen.history.top)
        return "\n".join(lines + [line.rstrip() for line in self.screen.display])
