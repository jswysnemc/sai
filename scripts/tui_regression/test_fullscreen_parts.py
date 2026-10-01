"""【全屏视图】【终端回归】命令与输出分段展开、悬停高亮、居中回到底部按钮，以及有后台工作时的退出确认。"""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
import re
import threading
import time
import unittest

from terminal import TerminalSession

HOVER_BG = "\x1b[48;5;236m"
LONG_COMMAND = " && \\\n".join(f"echo STEP-{index}" for index in range(12))


class CommandModel:
    """按用户问题依次调用 run_command 或后台命令，工具结束后回复一行文本。"""

    def __init__(self, background_command="sleep 300"):
        """创建本地模型替身；background_command 为后台测试命令，返回无。"""

        class Handler(BaseHTTPRequestHandler):
            """按最后一条用户消息与已有工具结果决定下一步。"""

            def log_message(self, *_args):
                """忽略访问日志；参数为日志内容，返回无。"""

            def do_POST(self):
                """返回一次完整的流式响应；无参数，返回无。"""
                request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                messages = request["messages"]
                last_user = max(i for i, m in enumerate(messages) if m.get("role") == "user")
                tag = re.findall(r"\b(RUN|BG)\b", str(messages[last_user].get("content")))
                tool_done = any(m.get("role") == "tool" for m in messages[last_user:])
                if tag and tag[-1] == "RUN" and not tool_done:
                    call = {"name": "run_command", "arguments": json.dumps({"command": LONG_COMMAND})}
                elif tag and tag[-1] == "BG" and not tool_done:
                    call = {"name": "background_command", "arguments": json.dumps({
                        "action": "start", "command": background_command, "label": "fixture-sleep"})}
                else:
                    call = None
                # 每轮工具调用使用不同 ID，重复 ID 会被请求投影拒绝
                call_id = f"call-{len(messages)}"
                delta = ({"tool_calls": [{"index": 0, "id": call_id, "type": "function", "function": call}]}
                         if call else {"content": "FINISHED"})
                finish = "tool_calls" if call else "stop"
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Connection", "close")
                self.end_headers()
                chunk = {"choices": [{"index": 0, "delta": delta, "finish_reason": finish}]}
                self.wfile.write(("data: " + json.dumps(chunk) + "\n\ndata: [DONE]\n\n").encode())

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)

    def __enter__(self):
        """启动服务；返回实例。"""
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        return self

    def __exit__(self, *_args):
        """关闭服务；参数为异常信息，返回无。"""
        self.server.shutdown()
        self.server.server_close()

    def config(self):
        """返回启用命令工具、YOLO 权限的合成配置；无参数。"""
        return {"active_provider": "fixture", "providers": [{"id": "fixture", "display_name": "Fixture",
            "base_url": f"http://127.0.0.1:{self.server.server_port}/v1", "api_key": "local-fixture",
            "models": ["fixture"], "default_model": "fixture"}], "tools": {"enabled": True},
            "session": {"auto_title_enabled": False}, "skills": {"enabled": False},
            "memory": {"enabled": False}, "permission": {"tui_mode": "yolo", "cli_mode": "yolo"}}


def mouse(terminal, column, row, kind=0, release=True):
    """在 0 起始坐标发送 SGR 鼠标事件；kind 为按键编码，release 为是否补发抬起，返回无。"""
    terminal.send(f"\x1b[<{kind};{column + 1};{row + 1}M".encode())
    if release:
        terminal.send(f"\x1b[<{kind};{column + 1};{row + 1}m".encode())


def lines(terminal):
    """返回当前屏幕行。"""
    return terminal.text().splitlines()


def idle(terminal):
    """判断输入框是否为空闲状态：最后几行非空行里出现空输入时的快捷键提示。"""
    tail = [line for line in lines(terminal) if line.strip()][-3:]
    return any(("Shift+Tab" in line and ("history" in line or "历史" in line)) for line in tail)


def row_of(terminal, needle):
    """返回首个包含 needle 的屏幕行号，找不到时为 None。"""
    return next((row for row, line in enumerate(lines(terminal)) if needle in line), None)


class FullscreenPartsTests(unittest.TestCase):
    """真实终端中验证分段展开、悬停与退出确认。"""

    def test_command_and_output_expand_separately_with_hover(self):
        """点命令只展开命令，点输出只展开输出；悬停整段铺底色；返回无。"""
        with CommandModel() as model, TerminalSession(model.config(), columns=100, rows=40) as terminal:
            terminal.send(b"RUN\r")
            terminal.wait_for(lambda: "FINISHED" in terminal.text(), timeout=20)
            terminal.send(b"\x0f")
            terminal.wait_for(lambda: "Ctrl+O" in lines(terminal)[0], timeout=5)
            terminal.wait_for(lambda: row_of(terminal, "STEP-0") is not None)
            self.assertIsNone(row_of(terminal, "STEP-6"), "命令应折叠")
            # 1. 悬停命令行：发出悬停底色
            command_row = row_of(terminal, "STEP-0")
            mark = len(terminal.raw)
            terminal.send(f"\x1b[<35;5;{command_row + 1}M".encode())
            terminal.wait_for(lambda: HOVER_BG.encode() in bytes(terminal.raw[mark:]), timeout=5)
            # 2. 点命令行：命令展开，输出不受影响
            mouse(terminal, 4, command_row)
            terminal.wait_for(lambda: row_of(terminal, "STEP-6") is not None, timeout=5)
            output_hint = [row for row, line in enumerate(lines(terminal))
                           if ("click to expand" in line or "点击展开" in line) and row > row_of(terminal, "STEP-11")]
            self.assertTrue(output_hint, "命令展开后输出仍应折叠:\n" + terminal.text())
            # 3. 点输出折叠提示：输出展开
            mouse(terminal, 6, output_hint[0])
            # 标题右侧的快捷键说明含“点击展开/收起”，只检查正文区
            terminal.wait_for(lambda: not any(("click to expand" in line or "点击展开" in line)
                                              for line in lines(terminal)[1:]), timeout=5)
            terminal.send(b"\x0f")

    def test_bottom_button_is_centered_above_the_composer(self):
        """离开底部后按钮居中贴在输入框上方，标题右侧不再出现新输出提示；返回无。"""
        with CommandModel() as model, TerminalSession(model.config(), columns=100, rows=24) as terminal:
            for turn in range(3):
                # 等上一轮彻底收尾、输入框回到空闲提示后再发送，否则回车会落在收尾帧上
                terminal.wait_for(lambda: idle(terminal), timeout=20)
                terminal.send(b"RUN\r")
                terminal.wait_for(lambda: terminal.text(history=True).count("FINISHED") > turn, timeout=20)
            terminal.send(b"\x0f")
            terminal.wait_for(lambda: "Ctrl+O" in lines(terminal)[0], timeout=5)
            for _ in range(4):
                mouse(terminal, 10, 5, kind=64, release=False)
            label = re.compile(r"↓ (Back to bottom|回到底部|New output|有新输出)")
            terminal.wait_for(lambda: any(label.search(line) for line in lines(terminal)), timeout=5)
            screen = lines(terminal)
            row = next(row for row, line in enumerate(screen) if label.search(line))
            match = label.search(screen[row])
            center = match.start() + (match.end() - match.start()) // 2
            self.assertLess(abs(center - 48), 4, f"按钮应水平居中: {screen[row]!r}")
            self.assertFalse(label.search(screen[0]), "标题不应再显示新输出提示")
            self.assertTrue(any("yolo" in line for line in screen[row + 1:]), "按钮下方应是输入框")
            terminal.send(b"\x0f")

    def test_exit_with_background_command_asks_and_stops_it(self):
        """有后台命令时退出弹出选择；选择全部停止后进程结束；返回无。"""
        self.assert_background_command_stops("sleep 300")

    def test_exit_waits_for_delayed_background_command_start(self):
        """模拟登录 shell 延迟启动，退出确认仍应终止实际后台进程；返回无。"""
        self.assert_background_command_stops("sleep 2; sleep 300")

    def assert_background_command_stops(self, command):
        """command 为后台测试命令；等待实际启动后验证退出确认与终止，返回无。"""
        with CommandModel(command) as model, TerminalSession(model.config(), columns=100, rows=30) as terminal:
            terminal.send(b"BG\r")
            terminal.wait_for(lambda: "FINISHED" in terminal.text(), timeout=20)
            # 1. 【终端回归】【进程就绪】任务登记先于 shell 执行，不以模型回复作为进程启动信号
            terminal.wait_for(lambda: bool(sleep_pids(terminal.root)), timeout=10)
            pids = sleep_pids(terminal.root)
            self.assertTrue(pids, "后台 sleep 应已启动")
            terminal.send(b"/exit\r")
            terminal.wait_for(lambda: "fixture-sleep" in terminal.text()
                              and ("Stop all and exit" in terminal.text() or "全部停止并退出" in terminal.text()),
                              timeout=10)
            self.assertTrue("keep commands running" in terminal.text() or "保留后台命令" in terminal.text())
            terminal.send(b"\r")
            deadline = time.monotonic() + 15
            while time.monotonic() < deadline and any(alive(pid) for pid in pids):
                terminal.pump(0.2)
            self.assertFalse(any(alive(pid) for pid in pids), "选择全部停止后后台命令应结束")


def sleep_pids(root):
    """root 为独立测试工作目录；返回该目录下的 sleep 300 进程，排除其他会话。"""
    found = []
    for entry in os.listdir("/proc"):
        if not entry.isdigit():
            continue
        try:
            with open(f"/proc/{entry}/cmdline", "rb") as handle:
                args = handle.read().split(b"\0")
                if (len(args) >= 2 and os.path.basename(args[0]) == b"sleep" and args[1] == b"300"
                        and os.readlink(f"/proc/{entry}/cwd") == os.fspath(root)):
                    found.append(int(entry))
        except OSError:
            continue
    return found


def alive(pid):
    """判断进程是否存活且不是僵尸；pid 为进程号，返回布尔值。"""
    try:
        with open(f"/proc/{pid}/stat") as handle:
            return handle.read().split()[2] != "Z"
    except OSError:
        return False


if __name__ == "__main__":
    unittest.main()
