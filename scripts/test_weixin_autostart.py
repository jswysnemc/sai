"""【微信网关】【端到端回归】隔离配置验证按需启动、接收刷新和发送重试，不访问真实微信。"""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import threading
import time
import unittest


@unittest.skipUnless(os.name == "posix", "Process group and XDG isolation require POSIX")
class WeixinAutostartTests(unittest.TestCase):
    """覆盖实际主程序、网关独立进程与模拟 iLink 接口。"""

    def execute(self, refresh, route_reply=False):
        """【微信网关】【运行场景】参数 refresh 决定是否拒绝旧凭证，返回记录并始终回收后台进程。"""
        requests = []
        messages = []
        polling = threading.Event()
        expired = threading.Event()
        delivered_context = threading.Event()
        stopping = threading.Event()
        reply_ready = threading.Event()
        reply_completed = threading.Event()
        gateway_requests = []

        class Handler(BaseHTTPRequestHandler):
            """按请求路径提供模型响应和微信收发接口。"""

            def log_message(self, *_args):
                """【微信网关】【测试日志】参数为日志字段，返回无；不输出普通 HTTP 日志。"""

            def do_POST(self):
                """【微信网关】【模拟接口】无参数；发送响应并记录真实请求。"""
                body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                if self.path == "/ilink/bot/getupdates":
                    polling.set()
                    expired.wait(0.2)
                    events = []
                    if refresh and expired.is_set() and not delivered_context.is_set():
                        delivered_context.set()
                        events = [{"message_type": 1, "from_user_id": "user", "context_token": "fresh",
                                   "item_list": [{"type": 1, "text_item": {"text": "/help"}}]}]
                    if route_reply and reply_ready.is_set() and not delivered_context.is_set():
                        records = list((root / "state/sai/gateways/weixin-origins").glob("*.json"))
                        if records:
                            source = json.loads(records[0].read_text())[0]["origin"]["session_id"]
                            delivered_context.set()
                            events = [{"message_type": 1, "from_user_id": "user", "context_token": "fresh",
                                       "item_list": [{"type": 1, "text_item": {"text": f"回复 {source}：继续处理"}}]}]
                    self.respond({"ret": 0, "msgs": events, "get_updates_buf": "fixture"})
                    if delivered_context.is_set():
                        time.sleep(0.1)
                    return
                if self.path == "/ilink/bot/sendmessage":
                    message = body["msg"]
                    messages.append(message)
                    text = message["item_list"][0].get("text_item", {}).get("text", "")
                    if refresh and "\n\nprogress-" in text and message["context_token"] == "old":
                        expired.set()
                        self.respond({"ret": -2, "errmsg": "prepare failed"})
                    else:
                        self.respond({"ret": 0})
                        if "\n\nprogress-" in text:
                            reply_ready.set()
                    return
                is_gateway = "<channel-context>" in json.dumps(body, ensure_ascii=False)
                if is_gateway and route_reply:
                    gateway_requests.append(body)
                    round_index = len(gateway_requests) - 1
                    self.send_response(200)
                    self.send_header("Content-Type", "text/event-stream")
                    self.send_header("Connection", "close")
                    self.end_headers()
                    if round_index == 0:
                        function = {"name": "session_probe", "arguments": json.dumps({"scope": "all"})}
                    elif round_index == 1:
                        records = list((root / "state/sai/gateways/weixin-origins").glob("*.json"))
                        source = json.loads(records[0].read_text())[0]["origin"]["session_id"]
                        function = {"name": "mesh_send", "arguments": json.dumps({"to": f"session:{source}", "text": "继续处理", "correlation_id": "weixin-reply"})}
                    else:
                        function = None
                        reply_completed.set()
                    delta = {"tool_calls": [{"index": 0, "id": f"gateway-{round_index}", "type": "function", "function": function}]} if function else {"content": "已转发"}
                    event = {"choices": [{"index": 0, "delta": delta, "finish_reason": None}]}
                    self.wfile.write(("data: " + json.dumps(event) + "\n\n").encode())
                    event = {"choices": [{"index": 0, "delta": {}, "finish_reason": "tool_calls" if function else "stop"}]}
                    self.wfile.write(("data: " + json.dumps(event) + "\n\ndata: [DONE]\n\n").encode())
                    return
                index = len(requests)
                requests.append(body)
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Connection", "close")
                self.end_headers()
                if index < 2:
                    delta = {"tool_calls": [{"index": 0, "id": f"send-{index}", "type": "function",
                             "function": {"name": "send_channel_message", "arguments": json.dumps({
                                 "kind": "text", "channel": "weixin", "text": f"progress-{index}"})}}]}
                    reason = "tool_calls"
                else:
                    if route_reply:
                        reply_completed.wait(10)
                    delta = {"content": "sending-complete"}
                    reason = "stop"
                for change, finish in [(delta, None), ({}, reason)]:
                    event = {"choices": [{"index": 0, "delta": change, "finish_reason": finish}]}
                    self.wfile.write(("data: " + json.dumps(event) + "\n\n").encode())
                self.wfile.write(b"data: [DONE]\n\n")

            def respond(self, body):
                """【微信网关】【JSON 响应】参数 body 为响应对象，返回无。"""
                data = json.dumps(body).encode()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                try:
                    self.wfile.write(data)
                except (BrokenPipeError, ConnectionResetError):
                    if not stopping.is_set():
                        raise

        with tempfile.TemporaryDirectory(prefix="sai-weixin-test-") as directory:
            root = Path(directory)
            server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            processes = root / "state/sai/gateways/processes.json"
            try:
                url = f"http://127.0.0.1:{server.server_port}"
                config = {
                    "provider_setup_complete": True, "active_provider": "fixture",
                    "providers": [{"id": "fixture", "display_name": "Fixture", "base_url": url + "/v1",
                                   "api_key": "fixture", "models": ["fixture"], "default_model": "fixture"}],
                    "agents": [{"id": "cli", "name": "test", "enabled_tools": ["send_channel_message"],
                                "deferred_tools": []}],
                    "gateways": {"weixin": {"enabled": False, "base_url": url, "cdn_base_url": url,
                                            "token": "fixture"}},
                    "skills": {"enabled": False}, "load_instruction_files": False,
                    "jev": {"routing": {"enabled": False}}, "codemode": {"mode": "off"},
                    "tools": {"command_shell": "/bin/sh"}, "display": {"fullscreen": False},
                }
                target = root / "config/sai/config.jsonc"
                target.parent.mkdir(parents=True)
                target.write_text(json.dumps(config))
                contexts = root / "state/sai/gateways/channel-contexts.json"
                contexts.parent.mkdir(parents=True)
                contexts.write_text(json.dumps({"weixin": {"channel": "weixin", "gateway": "weixin-server",
                                    "to_user_id": "user", "context_token": "old"}}))
                environment = dict(os.environ, TERM="dumb")
                for key, folder in [("XDG_CONFIG_HOME", "config"), ("XDG_STATE_HOME", "state"),
                                    ("XDG_DATA_HOME", "data"), ("XDG_CACHE_HOME", "cache")]:
                    environment[key] = str(root / folder)
                binary = Path(os.environ.get("SAI_TEST_BINARY", Path(__file__).resolve().parents[1] / "target/debug/sai")).resolve()
                result = subprocess.run([str(binary), "--lang", "en-US", "--yolo", "ask", "Send the progress to Weixin"],
                                        cwd=root, env=environment, capture_output=True, text=True, timeout=35)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertTrue(polling.is_set(), result.stdout + result.stderr)
                self.assertIn("sending-complete", result.stdout)
                self.assertTrue(processes.exists())
                records = json.loads(processes.read_text())
                gateways = [record for record in records if record["gateway_id"] == "weixin"]
                self.assertEqual(len(gateways), 1)
                self.assertTrue((root / "state/sai/gateways/weixin-runtime.json").exists())
                self.assertFalse(json.loads(target.read_text())["gateways"]["weixin"]["enabled"])
                outputs = [message["content"] for request in requests for message in request["messages"]
                           if message["role"] == "tool"]
                self.assertTrue(any('"ok": true' in text for text in outputs), outputs)
                sent = [message for message in messages if "\n\nprogress-" in message["item_list"][0].get("text_item", {}).get("text", "")]
                self.assertTrue(sent, messages)
                for message in sent:
                    text = message["item_list"][0]["text_item"]["text"]
                    self.assertTrue(text.startswith("[Sai · "), text)
                    self.assertIn("会话：", text)
                    self.assertIn("回复此会话：回复 ", text)
                self.assertTrue(list((root / "state/sai/gateways/weixin-origins").glob("*.json")))
                if route_reply:
                    self.assertTrue(reply_completed.is_set(), gateway_requests)
                    self.assertTrue(gateway_requests)
                    self.assertIn("recent-outbound-origins", json.dumps(gateway_requests[0]))
                    envelopes = [json.loads(line) for file in (root / "state/sai").rglob("mesh.jsonl") for line in file.read_text().splitlines()]
                    self.assertTrue(any(item["text"] == "继续处理" and item.get("correlation_id") == "weixin-reply" for item in envelopes), envelopes)
                    self.assertFalse(any(call["function"]["name"] == "session_create" for request in gateway_requests for message in request["messages"] for call in (message.get("tool_calls") or [])))
                return sent
            finally:
                stopping.set()
                if processes.exists():
                    for record in json.loads(processes.read_text()):
                        try:
                            pgid = record.get("pgid")
                            if pgid:
                                os.killpg(pgid, signal.SIGKILL)
                            else:
                                os.kill(record["pid"], signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                server.shutdown()
                server.server_close()
                thread.join()

    def test_stopped_gateway_starts_once_and_is_reused(self):
        """【微信网关】【启动复用】无参数，返回无；两次工具调用只启动一个网关且不修改偏好。"""
        sent = self.execute(False)
        self.assertEqual(len(sent), 2)

    def test_prepare_failure_refreshes_via_real_gateway_before_retry(self):
        """【微信网关】【真实刷新】无参数，返回无；长轮询更新凭证后仅重发被拒绝的消息。"""
        sent = self.execute(True)
        self.assertEqual(len(sent), 3)
        self.assertEqual([message["context_token"] for message in sent], ["old", "fresh", "fresh"])
        self.assertEqual(sent[0]["client_id"], sent[1]["client_id"])

    def test_weixin_reply_is_forwarded_back_to_the_source_session(self):
        """【微信网关】【来源回复】无参数，返回无；来源记录进入网关背景，探测后投递到原会话。"""
        self.execute(False, route_reply=True)


if __name__ == "__main__":
    unittest.main()
