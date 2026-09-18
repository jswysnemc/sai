# /// script
# requires-python = ">=3.11"
# dependencies = ["pyte>=0.8.2,<0.9"]
# ///
"""在共享会话中启动两个真实 TUI，验证输入本地执行且不再自动跟随或接管。"""

import argparse
import json
import sqlite3
import sys
import tempfile
import termios
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from fixture import Fixture
from verify_tui_status import Terminal


class Provider(BaseHTTPRequestHandler):
    """只接收隔离验收发出的请求，返回可识别的本地模型响应。"""

    requests = []
    slow_started = threading.Event()
    release_slow = threading.Event()

    def log_message(self, *_args):
        """【本地会话验收】【安静日志】忽略默认 HTTP 输出，无返回值。"""

    def do_POST(self):
        """【本地会话验收】【模拟模型】读取当前请求，返回带原始消息标记的 SSE 答复。"""
        payload = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        text = next(message["content"] for message in reversed(payload["messages"]) if message["role"] == "user")
        text = text.rstrip().splitlines()[-1]
        if not payload.get("stream"):
            data = json.dumps({"choices":[{"message":{"role":"assistant","content":"fixture"},"finish_reason":"stop"}]}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
            return
        memory_request = any(message.get("role") == "system" and "session memory extraction worker" in str(message.get("content")) for message in payload["messages"])
        if not memory_request:
            self.requests.append(text)
        if text == "left-slow":
            self.slow_started.set()
            assert self.release_slow.wait(timeout=15)
        response = "ack-" + text
        frames = [
            {"id":"fixture", "choices":[{"index":0,"delta":{"role":"assistant","content":response},"finish_reason":None}]},
            {"id":"fixture", "choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}},
        ]
        body = "".join("data: " + json.dumps(frame) + "\n\n" for frame in frames) + "data: [DONE]\n\n"
        data = body.encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


def pump(terminals, predicate, timeout=12):
    """【本地会话验收】【并行读屏】接收终端及条件，直到成立或超时，返回无。"""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        for terminal in terminals:
            terminal.read(0.01)
        if predicate():
            return
    raise AssertionError("Condition timed out\n" + "\n---\n".join(item.display() for item in terminals))



def ready(terminal):
    """【本地会话验收】【输入就绪】接收终端，确认原始输入模式启用后已绘制输入框。"""
    raw = terminal.raw
    enabled = raw.rfind(b"\x1b[?2004h")
    return (enabled >= 0 and enabled > raw.rfind(b"\x1b[?2004l")
            and raw.rfind(b"\x1b[?2026l") > enabled
            and not termios.tcgetattr(terminal.master)[3] & termios.ICANON)

def verify(fixture):
    """【本地会话验收】【双终端】接收隔离环境，验证旧登记无效和两个终端独立执行，返回屏幕证据。"""
    server = ThreadingHTTPServer(("127.0.0.1", 0), Provider)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    config_dir = fixture.root / "config/sai"
    config_dir.mkdir(parents=True)
    (config_dir / "config.jsonc").write_text(json.dumps({
        "active_provider":"fixture", "providers":[{"id":"fixture","display_name":"Fixture",
        "base_url":f"http://127.0.0.1:{server.server_port}/v1", "api_key":"fixture", "default_model":"fixture-model"}],
        "tools":{"enabled":False}, "skills":{"enabled":False}, "memory":{"enabled":False},
        "session":{"auto_title_enabled":False}, "load_instruction_files":False,
        "retry":{"max_attempts":1},
    }), encoding="utf-8")
    terminals = []
    screens = {}
    try:
        left = Terminal(fixture)
        terminals.append(left)
        pump(terminals, lambda: ready(left) and "Add a follow-up" in left.display())
        records = list((fixture.root / "state").rglob("session-presence/*.json"))
        assert len(records) == 1
        state_dir = records[0].parent.parent
        record = json.loads(records[0].read_text())
        legacy = state_dir / "session-holder.json"
        legacy.write_text(json.dumps({"schema":1,"session_id":record["session_id"], "owner":"repl",
            "pid":left.process.pid,"started_at":record["started_at"],"heartbeat_at":record["started_at"],
            "watchers":0,"transport":None}), encoding="utf-8")
        legacy_bytes = legacy.read_bytes()
        left.send("left-slow")
        pump(terminals, lambda: "→ left-slow" in left.display())
        left.send("\r")
        pump(terminals, Provider.slow_started.is_set)
        right = Terminal(fixture)
        terminals.append(right)
        pump(terminals, lambda: ready(right))
        instances = list(state_dir.glob("session-presence/*.json"))
        assert len(instances) == 2
        assert {json.loads(path.read_text())["pid"] for path in instances} == {item.process.pid for item in terminals}
        with sqlite3.connect(state_dir / "conversation.db") as connection:
            assert connection.execute("SELECT status FROM turns WHERE user_content = 'left-slow'").fetchone() == ("running",)
        Provider.release_slow.set()
        pump(terminals, lambda: "ack-left-slow" in left.display() and not (state_dir / "active-run.json").exists())
        assert "ack-left-slow" not in right.display(), right.display()
        right.send("right-local")
        pump(terminals, lambda: "→ right-local" in right.display())
        right.send("\r")
        pump(terminals, lambda: "ack-right-local" in right.display() and not (state_dir / "active-run.json").exists())
        assert "right-local" not in left.display(), left.display()
        screens["left"] = left.display()
        screens["right"] = right.display()
        forbidden = ["driven by another", "holder is unreachable", "follower", "takes over", "接管", "跟随中"]
        assert not any(word in screen for word in forbidden for screen in screens.values())
        assert legacy.read_bytes() == legacy_bytes
        left.close()
        terminals.remove(left)
        pump(terminals, lambda: ready(right))
        right.send("after-close")
        pump(terminals, lambda: "→ after-close" in right.display())
        right.send("\r")
        pump(terminals, lambda: "ack-after-close" in right.display())
        pump(terminals, lambda: not (state_dir / "active-run.json").exists())
        with sqlite3.connect(state_dir / "conversation.db") as connection:
            turns = connection.execute("SELECT user_content, assistant_content, status FROM turns ORDER BY seq").fetchall()
        assert turns == [(text, "ack-" + text, "completed") for text in ["left-slow", "right-local", "after-close"]], turns
        screens["after_close"] = right.display()
        assert Provider.requests == ["left-slow", "right-local", "after-close"], Provider.requests
        assert not any(word in screens["after_close"] for word in forbidden)
        return {"screens":screens,"requests":Provider.requests,"same_session":record["session_id"]}
    finally:
        Provider.release_slow.set()
        for terminal in terminals:
            terminal.close()
        server.shutdown()
        server.server_close()


def main():
    """【本地会话验收】【脚本入口】读取程序路径，在 Linux 隔离目录执行并输出证据，无返回值。"""
    if sys.platform != "linux":
        raise SystemExit("This isolated XDG/PTY smoke test requires Linux")
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=Path("target/debug/sai"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="sai-local-sessions-") as temporary:
        report = verify(Fixture(args.binary.resolve(), Path(temporary)))
    args.output.write_text(json.dumps({"ok":True, **report}, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"ok":True,"requests":report["requests"],"output":str(args.output)}))


if __name__ == "__main__":
    main()
