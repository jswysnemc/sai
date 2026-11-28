"""【终端】【群组工作台回归】端到端验证群组 TUI 原生风格、模型信息展示、真实 Leader/Runner 编排全链路与浅色主题对比度。"""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import re
import threading
import unittest

from terminal import TerminalSession


def luminance(color):
    """计算六位十六进制 RGB 的 sRGB 相对亮度。"""
    channels = [int(color[i : i + 2], 16) / 255 for i in (0, 2, 4)]
    channels = [
        v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4 for v in channels
    ]
    return sum(v * w for v, w in zip(channels, (0.2126, 0.7152, 0.0722)))


class MockSuperAgentModel:
    """支持直接对话与完整 Leader -> Runner -> Report -> Delivery 编排全链路的本地模型替身。"""

    def __init__(self, orchestrate=False):
        self.orchestrate = orchestrate
        self.requests = []
        self.lock = threading.Lock()
        model = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_):
                """忽略常规 HTTP 日志。"""

            def do_POST(self):
                length = int(self.headers.get("Content-Length", 0))
                raw = self.rfile.read(length)
                payload = json.loads(raw.decode("utf-8"))
                with model.lock:
                    model.requests.append(payload)
                messages = payload.get("messages", [])
                system_text = "\n".join(
                    str(m.get("content", ""))
                    for m in messages
                    if isinstance(m, dict) and m.get("role") == "system"
                )
                last_role = messages[-1].get("role", "") if messages else ""
                is_worker = "<super-agent-worker-role>" in system_text

                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Connection", "close")
                self.end_headers()
                try:
                    if not model.orchestrate:
                        self.emit({"content": "**收到任务**：Leader 已完成评估，随时可派发 Runner。"})
                        self.emit({}, "stop")
                    elif is_worker:
                        if last_role == "tool":
                            self.emit({"content": "巡检报告已提交给 Leader。"})
                            self.emit({}, "stop")
                        else:
                            self.emit_tool_call(
                                "call_worker_rep",
                                "super_report_submit",
                                {
                                    "status": "ready",
                                    "summary": "系统健康检查通过",
                                    "details": "已完成工作区巡检，无异常告警。",
                                    "artifacts": ["Cargo.toml"],
                                },
                            )
                            self.emit({}, "tool_calls")
                    else:
                        all_user = "\n".join(
                            str(m.get("content", ""))
                            for m in messages
                            if isinstance(m, dict) and m.get("role") == "user"
                        )
                        if last_role == "tool":
                            self.emit({"content": "已完成当前编排步骤。"})
                            self.emit({}, "stop")
                        elif "<runner-reports" in all_user:
                            self.emit_tool_call(
                                "call_leader_deliver",
                                "super_deliver_result",
                                {
                                    "summary": "检查完成：系统全部正常",
                                    "deliverable": "## 最终交付\nRunner `inspector` 已完成巡检，系统健康无告警。",
                                },
                            )
                            self.emit({}, "tool_calls")
                        else:
                            self.emit_tool_call(
                                "call_leader_spawn",
                                "super_spawn_runner",
                                {
                                    "name": "inspector",
                                    "specialty": "diagnostics",
                                    "task_title": "系统健康巡检",
                                    "prompt": "请执行系统健康检查并汇报结果",
                                },
                            )
                            self.emit({}, "tool_calls")
                    self.wfile.write(b"data: [DONE]\n\n")
                    self.wfile.flush()
                except (BrokenPipeError, ConnectionResetError):
                    pass

            def emit(self, delta, finish=None):
                data = {
                    "id": "group-fixture",
                    "object": "chat.completion.chunk",
                    "choices": [{"index": 0, "delta": delta, "finish_reason": finish}],
                }
                self.wfile.write(("data: " + json.dumps(data) + "\n\n").encode())
                self.wfile.flush()

            def emit_tool_call(self, call_id, name, arguments):
                self.emit(
                    {
                        "tool_calls": [
                            {
                                "index": 0,
                                "id": call_id,
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": json.dumps(arguments, ensure_ascii=False),
                                },
                            }
                        ]
                    }
                )

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)

    def __enter__(self):
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        return self

    def __exit__(self, *_):
        self.server.shutdown()
        self.server.server_close()

    def config(self):
        return {
            "active_provider": "fixture",
            "providers": [
                {
                    "id": "fixture",
                    "display_name": "Fixture",
                    "base_url": f"http://127.0.0.1:{self.server.server_port}/v1",
                    "api_key": "local-fixture",
                    "models": ["fixture-pro"],
                    "default_model": "fixture-pro",
                    "thinking_level": "high",
                }
            ],
            "super_agent": {"worker_use_worktree": False},
            "tools": {"enabled": True},
            "skills": {"enabled": False},
        }


class GroupTuiRegressionTests(unittest.TestCase):
    """验证 `sai group` 在真实 PTY 下的原生风格、模型展示、对话闭环、编排全链路与浅色主题可读性。"""

    def test_group_chat_native_composer_model_info_and_interactions(self):
        with MockSuperAgentModel(orchestrate=False) as model:
            with TerminalSession(
                config=model.config(),
                columns=132,
                rows=34,
                arguments=["group"],
                ready=lambda term: "群组成员" in term.text() and "fixture-pro" in term.text(),
            ) as term:
                initial = term.text()
                # 1. 验证采用 Sai 原生品牌头与输入区（→ 提示符、无割裂圆角输入框）且展示模型、思考等级与上下文
                self.assertIn("╭──╴", initial)
                self.assertIn("→", initial)
                self.assertNotIn("发言 ·", initial)
                self.assertIn("fixture-pro", initial)
                self.assertIn("high", initial)
                self.assertIn("CTX", initial)
                self.assertFalse(term.screen.cursor.hidden, "主输入框必须显示真实终端光标")

                # 2. 发送真实群组消息给 Leader，验证闭环无报错
                term.send("请检查项目架构\r".encode("utf-8"))
                term.wait_for(lambda: "收到任务" in term.text(), timeout=15)
                screen = term.text()
                self.assertNotIn("session not found", screen)
                self.assertNotIn("Leader 执行失败", screen)
                self.assertIn("收到任务", screen)

                # 3. 校验 Leader 真实发送给模型的工具定义：含 9 个 super_* 工具，不含重执行工具
                self.assertTrue(model.requests, "expected at least one LLM request from Leader")
                tool_names = {
                    tool["function"]["name"]
                    for tool in model.requests[-1].get("tools", [])
                    if isinstance(tool, dict) and isinstance(tool.get("function"), dict)
                }
                for expected in [
                    "super_list_state",
                    "super_spawn_runner",
                    "super_dispatch_task",
                    "super_deliver_result",
                    "super_recall",
                ]:
                    self.assertIn(expected, tool_names)
                for forbidden in ["run_command", "write_file", "apply_patch"]:
                    self.assertNotIn(forbidden, tool_names)

                # 4. 切换右侧检视标签页：渠道页展示微信接入步骤，工具页展示 Leader 编排工具
                term.send(b"\t")
                term.wait_for(lambda: "weixin-login" in term.text(), timeout=5)
                term.send(b"\t")
                term.wait_for(lambda: "super_spawn_runner" in term.text(), timeout=5)

                # 5. 触发透明内联斜杠补全并用 Tab 补全 `/tools`（精确匹配自动收起面板），单次 Esc 清空输入且不退出
                term.send(b"/to")
                term.wait_for(lambda: "/tools" in term.text(), timeout=5)
                term.send(b"\t")
                term.wait_for(lambda: "→ /tools" in term.text(), timeout=5)
                term.send(b"\x1b")
                term.wait_for(lambda: "→ /tools" not in term.text(), timeout=5)
                self.assertIn("fixture-pro", term.text())

                # 6. 按 F1 打开帮助弹窗，确认居中于主画布不截断右侧栏、隐藏光标且包含模型说明，再按 Esc 关闭
                term.send(b"\x1bOP")
                term.wait_for(lambda: "群组工作台帮助" in term.text(), timeout=5)
                help_screen = term.text()
                self.assertIn("fixture-pro", help_screen)
                self.assertIn("/model", help_screen)
                self.assertIn("Leader", help_screen)
                self.assertIn("CTX", help_screen)
                self.assertTrue(term.screen.cursor.hidden, "只读帮助弹窗应隐藏硬件光标")
                term.send(b"\x1b")
                term.wait_for(lambda: "群组工作台帮助" not in term.text(), timeout=5)
                self.assertFalse(term.screen.cursor.hidden, "关闭弹窗后应恢复主输入光标")

                # 7. 按 Ctrl+R 打开启动 Runner 表单弹窗，确认激活字段显示真实硬件光标，再按 Esc 关闭
                term.send(b"\x12")
                term.wait_for(lambda: "启动异步 Runner" in term.text(), timeout=5)
                self.assertFalse(term.screen.cursor.hidden, "表单弹窗激活字段应显示真实硬件光标")
                term.send(b"\x1b")
                term.wait_for(lambda: "启动异步 Runner" not in term.text(), timeout=5)

    def test_leader_spawns_runner_and_synthesizes_delivery_in_real_pty(self):
        with MockSuperAgentModel(orchestrate=True) as model:
            with TerminalSession(
                config=model.config(),
                columns=132,
                rows=36,
                arguments=["group"],
                ready=lambda term: "群组成员" in term.text() and "fixture-pro" in term.text(),
            ) as term:
                term.send("帮我检查系统状态并汇报\r".encode("utf-8"))
                term.wait_for(
                    lambda: "最终交付" in term.text() and "✓ 已归纳" in term.text(),
                    timeout=20,
                )
                term.pump(1.0)
                screen = term.text()
                self.assertNotIn("session not found", screen)
                self.assertNotIn("Leader 执行失败", screen)
                self.assertNotIn("系统告警", screen)
                self.assertIn("inspector", screen)
                self.assertIn("成果交付", screen)
                self.assertIn("系统健康检查通过", screen)
                self.assertIn("✓ 已归纳", screen)
                self.assertIn("✓ 系统健康巡检", screen)

    def test_light_theme_contrast_and_narrow_fallback(self):
        background = "c2d5e7"

        def assert_screen_contrast(term, label):
            checked = 0
            for row in term.screen.buffer.values():
                for cell in row.values():
                    if not cell.data.strip() or not re.fullmatch(r"[0-9a-fA-F]{6}", cell.fg):
                        continue
                    bg_hex = cell.bg if re.fullmatch(r"[0-9a-fA-F]{6}", cell.bg) else background
                    fg_l, bg_l = luminance(cell.fg), luminance(bg_hex)
                    ratio = (max(fg_l, bg_l) + 0.05) / (min(fg_l, bg_l) + 0.05)
                    self.assertGreaterEqual(
                        ratio,
                        4.45,
                        f"[{label}] Low contrast cell {cell.data!r}: fg={cell.fg} bg={bg_hex} ratio={ratio:.2f}",
                    )
                    checked += 1
            self.assertGreater(checked, 60)

        with MockSuperAgentModel(orchestrate=False) as model:
            with TerminalSession(
                config=model.config(),
                columns=128,
                rows=34,
                arguments=["group"],
                terminal_colors=("202020", background),
                ready=lambda term: "群组成员" in term.text() and "fixture-pro" in term.text(),
            ) as term:
                # 1. 验证浅色终端上初始视图、斜杠补全面板与帮助弹窗全部文本均满足可读对比度
                assert_screen_contrast(term, "initial")
                term.send(b"/")
                term.wait_for(lambda: "快捷命令" in term.text(), timeout=5)
                assert_screen_contrast(term, "slash-completion")
                term.send(b"\x1b")
                term.wait_for(lambda: "快捷命令" not in term.text(), timeout=5)
                term.send(b"\x1b")
                term.wait_for(lambda: "→ /" not in term.text(), timeout=5)

                term.send(b"\x1bOP")
                term.wait_for(lambda: "群组工作台帮助" in term.text(), timeout=5)
                assert_screen_contrast(term, "help-modal")
                term.send(b"\x1b")
                term.wait_for(lambda: "群组工作台帮助" not in term.text(), timeout=5)

                # 2. 窄屏自适应收起侧栏，顶栏进度贴齐右缘且底栏仍保留模型基本信息
                term.resize(84, 24)
                term.wait_for(lambda: "CTX" not in term.text(), timeout=5)
                narrow = term.text()
                self.assertIn("fixture-pro", narrow)
                self.assertIn("→", narrow)
                row0 = term.screen.display[0]
                self.assertTrue(
                    row0.endswith("100% "),
                    f"narrow header should not leave dead toggle gap: {row0!r}",
                )


if __name__ == "__main__":
    unittest.main()
