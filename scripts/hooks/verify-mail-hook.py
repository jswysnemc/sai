"""【Hooks】【真实验证】隔离模型接口触发 Stop，只有显式 --send 才连接用户邮箱。"""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading


class ModelHandler(BaseHTTPRequestHandler):
    """【Hooks】【模型夹具】本地接口返回确定回复，不调用付费模型。"""

    def log_message(self, *_args):
        """参数为访问日志；返回无值，不输出请求或凭据。"""

    def do_POST(self):
        """无参数；返回单次兼容 SSE 回复，供 Agent 触发真实完成路径。"""
        self.rfile.read(int(self.headers["Content-Length"]))
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Connection", "close")
        self.end_headers()
        result = {"choices": [{"index": 0, "delta": {"content": "MAIL-HOOK-VERIFIED"}, "finish_reason": "stop"}]}
        self.wfile.write(("data: " + json.dumps(result) + "\n\ndata: [DONE]\n\n").encode())


def main() -> int:
    """参数来自 --binary 和 --send；返回 Agent 与同步 Stop 邮件动作的验证退出码。"""
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--send", action="store_true")
    arguments = parser.parse_args()
    if not arguments.send:
        parser.error("Use --send to explicitly authorize one real notification")
    server = ThreadingHTTPServer(("127.0.0.1", 0), ModelHandler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        with tempfile.TemporaryDirectory(prefix="sai-mail-verification-") as directory:
            root = Path(directory)
            environment = dict(os.environ)
            for name, child in [("XDG_CONFIG_HOME", "config"), ("XDG_DATA_HOME", "data"), ("XDG_STATE_HOME", "state"), ("XDG_CACHE_HOME", "cache")]:
                environment[name] = str(root / child)
            environment["SAI_QUIET"] = "1"
            config = {"active_provider": "fixture", "providers": [{"id": "fixture", "display_name": "Local fixture", "base_url": f"http://127.0.0.1:{server.server_port}/v1", "api_key": "local-fixture", "models": ["fixture"], "default_model": "fixture"}],
                "tools": {"enabled": False}, "skills": {"enabled": False}, "plugins": {"memory": {"enabled": False}}, "session": {"auto_title_enabled": False},
                "hooks": {"enabled": True, "items": [{"name": "mail-verification", "event": "Stop", "kind": "command", "script": f"python3 '{Path(__file__).with_name('mail-notify.py').resolve()}'", "timeout_ms": 60000}]}}
            file = root / "config/sai/config.jsonc"
            file.parent.mkdir(parents=True)
            file.write_text(json.dumps(config))
            # 1. 【Hooks】【验证隔离】同步模式确保退出前完成发送，常驻用户配置仍使用异步通知
            result = subprocess.run([str(Path(arguments.binary).resolve()), "--lang", "en-US", "ask", "Verify mail notification once"], cwd=root, env=environment, text=True, capture_output=True, timeout=120, check=False)
            history = root / "data/sai/hook-history/events.jsonl"
            records = [json.loads(line) for line in history.read_text().splitlines()] if history.exists() else []
            completed = [record for record in records if record["name"] == "mail-verification" and record["status"] == "completed"]
            failures = [record for record in records if record["status"] == "failed"]
            if result.returncode != 0 or "MAIL-HOOK-VERIFIED" not in result.stdout or len(completed) != 1 or failures:
                print(f"Mail verification failed: exit={result.returncode}, completed={len(completed)}, failures={len(failures)}")
                for record in failures:
                    print(record.get("error", "Hook failure"))
                return 1
            print("Agent Stop hook completed once; local mailer returned success. SMTP acceptance does not confirm inbox delivery.")
            return 0
    finally:
        server.shutdown()
        server.server_close()


if __name__ == "__main__":
    raise SystemExit(main())
