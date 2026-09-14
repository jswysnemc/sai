"""【会话载入】【终端测量】测量隔离会话恢复时的绘制耗时和输出量。"""

import os
import select
import signal
import sqlite3
import time


def measure_tui_history(binary, workspace, state, process_env):
    """测量恢复首屏；参数为程序、工作区、隔离状态和环境，返回耗时与输出字节数。"""
    import fcntl
    import pty
    import struct
    import termios
    import pyte

    marker = "SAISESSIONREADY"
    database = state / "data/default/conversation.db"
    with sqlite3.connect(database) as connection:
        connection.execute(
            "UPDATE turns SET assistant_content = assistant_content || ? WHERE seq = (SELECT MAX(seq) FROM turns)",
            ("\n" + marker,),
        )
        oldest_user = connection.execute(
            "SELECT user_content FROM turns ORDER BY seq DESC LIMIT 50"
        ).fetchall()[-1][0]
    environment = dict(process_env, TERM="xterm-256color", COLORTERM="truecolor")
    executable = str(binary.resolve())
    started = time.perf_counter()
    pid, fd = pty.fork()
    if pid == 0:
        os.chdir(workspace)
        os.execve(executable, [executable, "resume", "default"], environment)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 32, 100, 0, 0))
    screen = pyte.Screen(100, 32)
    stream = pyte.Stream(screen)
    output_bytes = 0
    parser_seconds = 0.0
    pending = b""
    try:
        deadline = time.monotonic() + 30
        # 1. 【会话载入】【终端协议】响应真实光标与背景色查询，避免探测超时污染测量
        while time.monotonic() < deadline:
            readable, _, _ = select.select([fd], [], [], 0.1)
            if not readable:
                continue
            try:
                data = os.read(fd, 65536)
            except OSError:
                break
            if not data:
                break
            output_bytes += len(data)
            parsing = time.perf_counter()
            stream.feed(data.decode("utf-8", "replace"))
            parser_seconds += time.perf_counter() - parsing
            if b"\x1b[6n" in data:
                os.write(fd, f"\x1b[{screen.cursor.y + 1};{screen.cursor.x + 1}R".encode())
            if b"\x1b]11;?" in data:
                os.write(fd, b"\x1b]11;rgb:0000/0000/0000\x1b\\")
            if b"integrate" in data.lower() or "是否集成" in data.decode("utf-8", "ignore"):
                os.write(fd, b"n\r")
            pending = pending[-64:] + data
            if marker.encode() in pending:
                result = {
                    "median_ms": round((time.perf_counter() - started) * 1000, 3),
                    "terminal_parser_ms": round(parser_seconds * 1000, 3),
                    "output_bytes": output_bytes,
                }
                verify_history_pager(fd, stream, screen, oldest_user)
                result["history_verified"] = True
                return result
        raise RuntimeError("Terminal history did not become visible:\n" + "\n".join(screen.display))
    finally:
        # 2. 【会话载入】【隔离清理】只终止本次创建的测试进程
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        os.waitpid(pid, 0)
        os.close(fd)


def verify_history_pager(fd, stream, screen, expected):
    """验证完整回看；参数为伪终端、屏幕解析器、屏幕及旧消息，成功返回无。"""
    os.write(fd, b"\x0f")
    pending = b""
    switched = False
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        readable, _, _ = select.select([fd], [], [], 0.1)
        if not readable:
            continue
        try:
            data = os.read(fd, 65536)
        except OSError:
            break
        if not data:
            break
        stream.feed(data.decode("utf-8", "replace"))
        if b"\x1b[6n" in data:
            os.write(fd, f"\x1b[{screen.cursor.y + 1};{screen.cursor.x + 1}R".encode())
        pending = pending[-16384:] + data
        if b"Segment" in pending and not switched:
            os.write(fd, b"a")
            switched = True
        if expected.encode() in pending:
            return
    raise RuntimeError("Earlier history was unavailable in Ctrl+O:\n" + "\n".join(screen.display))
