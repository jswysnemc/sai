"""【Hooks】【邮箱测试】以临时邮件脚本验证通知协议，不连接 SMTP。"""

from contextlib import redirect_stderr, redirect_stdout
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


def load_script(name):
    """【Hooks】【脚本夹具】name 为同目录脚本文件名，返回不执行命令入口的模块"""
    spec = importlib.util.spec_from_file_location(
        "hook_fixture", Path(__file__).with_name(name)
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class MailNotifyTests(unittest.TestCase):
    """【Hooks】【通知夹具】验证有效完成事件与无关事件。"""

    def test_stop_payload_and_output_separation(self):
        """【Hooks】【通知隔离】无参数，Stop 事件发送一次，终端没有诊断输出，返回无值"""
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            output = root / "arguments.json"
            mailer = root / "mailer.py"
            mailer.write_text(
                f"import sys, json\nfrom pathlib import Path\nPath({str(output)!r}).write_text(json.dumps(sys.argv[1:]))\nprint('SMTP accepted')\n"
            )
            script = Path(__file__).with_name("mail-notify.py")
            event = {
                "hook_event_name": "Stop",
                "session_id": "session-test",
                "turn_id": "turn-test",
                "cwd": str(root),
                "last_assistant_message": "completed",
            }
            log = root / "mail-notify.log"
            environment = {**os.environ, "SAI_MAIL_LOG": str(log)}
            result = subprocess.run(
                [sys.executable, str(script), "--mailer", str(mailer)],
                input=json.dumps(event),
                text=True,
                capture_output=True,
                env=environment,
                check=False,
            )
            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "")
            self.assertEqual(result.stderr, "")
            record = json.loads(log.read_text())
            self.assertEqual(record["status"], "sent")
            self.assertEqual(record["level"], "info")
            self.assertIn("SMTP accepted", record["stdout"])
            self.assertIn("completed", json.loads(output.read_text())[-1])
            output.unlink()
            event["hook_event_name"] = "Interrupt"
            result = subprocess.run(
                [sys.executable, str(script), "--mailer", str(mailer)],
                input=json.dumps(event),
                text=True,
                capture_output=True,
                env=environment,
                check=False,
            )
            self.assertEqual(result.returncode, 0)
            self.assertFalse(output.exists())

    def test_smtp_failure_and_terminal_controls_only_reach_private_log(self):
        """【Hooks】【邮件失败回归】无参数，SMTP 失败及终端控制字符只进入私有日志，不影响后台状态和渲染，返回无值"""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            mailer = root / "mailer.py"
            mailer.write_text(
                "import sys\nprint('\\x1b[2JSMTP debug output')\nprint('Traceback: SMTPServerDisconnected: Connection unexpectedly closed', file=sys.stderr)\nraise SystemExit(1)\n"
            )
            log = root / "mail-notify.log"
            event = {"hook_event_name": "Stop", "session_id": "test", "turn_id": "turn"}
            result = subprocess.run(
                [
                    sys.executable,
                    str(Path(__file__).with_name("mail-notify.py")),
                    "--mailer",
                    str(mailer),
                ],
                input=json.dumps(event),
                text=True,
                capture_output=True,
                env={**os.environ, "SAI_MAIL_LOG": str(log)},
                check=False,
            )
            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "")
            self.assertEqual(result.stderr, "")
            record = json.loads(log.read_text())
            self.assertEqual(record["status"], "failed")
            self.assertEqual(record["level"], "error")
            self.assertIn("Connection unexpectedly closed", record["stderr"])
            self.assertIn("\x1b[2J", record["stdout"])
            self.assertEqual(log.stat().st_mode & 0o777, 0o600)

    def test_timeout_is_logged_without_command_arguments_or_terminal_output(self):
        """【Hooks】【超时隔离】无参数，邮件超时只记录时限，不把命令中的回复正文写入日志或终端，返回无值"""
        module = load_script("mail-notify.py")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            mailer = root / "mailer.py"
            mailer.write_text("")
            log = root / "mail-notify.log"
            event = {
                "hook_event_name": "Stop",
                "session_id": "test",
                "turn_id": "turn",
                "last_assistant_message": "private message",
            }
            output, error = io.StringIO(), io.StringIO()
            with (
                patch.dict(os.environ, {"SAI_MAIL_LOG": str(log)}),
                patch.object(sys, "argv", ["mail-notify.py", "--mailer", str(mailer)]),
                patch.object(sys, "stdin", io.StringIO(json.dumps(event))),
                patch.object(
                    module.subprocess,
                    "run",
                    side_effect=subprocess.TimeoutExpired(
                        ["mailer", "private message"], 45
                    ),
                ),
                redirect_stdout(output),
                redirect_stderr(error),
            ):
                self.assertEqual(module.main(), 0)
            self.assertEqual(output.getvalue(), "")
            self.assertEqual(error.getvalue(), "")
            record = json.loads(log.read_text())
            self.assertEqual(record["status"], "failed")
            self.assertEqual(record["timeout_seconds"], 45)
            self.assertNotIn("private message", log.read_text())

    def test_bad_input_and_missing_mailer_do_not_escape_to_terminal(self):
        """【Hooks】【输入隔离】无参数，无效 JSON、过大输入及缺失邮件脚本都只记录失败，返回无值"""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for payload in (
                "{",
                "[]",
                "x" * 1_048_577,
                json.dumps(
                    {"hook_event_name": "Stop", "session_id": "test", "turn_id": "turn"}
                ),
            ):
                log = root / "mail-notify.log"
                with self.subTest(payload_length=len(payload)):
                    result = subprocess.run(
                        [
                            sys.executable,
                            str(Path(__file__).with_name("mail-notify.py")),
                            "--mailer",
                            str(root / "missing.py"),
                        ],
                        input=payload,
                        text=True,
                        capture_output=True,
                        env={**os.environ, "SAI_MAIL_LOG": str(log)},
                        check=False,
                    )
                    self.assertEqual(result.returncode, 0)
                    self.assertEqual(result.stdout, "")
                    self.assertEqual(result.stderr, "")
                    self.assertEqual(json.loads(log.read_text())["status"], "failed")
                    log.unlink()

    def test_unwritable_log_does_not_reintroduce_terminal_diagnostics(self):
        """【Hooks】【日志异常隔离】无参数，即使日志不可写，通知失败也不重新输出到终端，返回无值"""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            blocked = root / "blocked"
            blocked.write_text("not a directory")
            event = {"hook_event_name": "Stop", "session_id": "test", "turn_id": "turn"}
            result = subprocess.run(
                [
                    sys.executable,
                    str(Path(__file__).with_name("mail-notify.py")),
                    "--mailer",
                    str(root / "missing.py"),
                ],
                input=json.dumps(event),
                text=True,
                capture_output=True,
                env={**os.environ, "SAI_MAIL_LOG": str(blocked / "mail-notify.log")},
                check=False,
            )
            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "")
            self.assertEqual(result.stderr, "")

    def test_delivery_verification_does_not_treat_handled_failure_as_success(self):
        """【Hooks】【发送结果回归】无参数，验证脚本必须读取实际发送记录，不能把静默处理失败当作已发送，返回无值"""
        module = load_script("verify-mail-hook.py")
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "mail-notify.log"
            self.assertEqual(module.delivery_counts(log), (0, 0))
            log.write_text(json.dumps({"status": "failed"}) + "\n")
            self.assertEqual(module.delivery_counts(log), (0, 1))
            log.write_text(json.dumps({"status": "sent"}) + "\n")
            self.assertEqual(module.delivery_counts(log), (1, 0))
            log.write_text("invalid log\n")
            self.assertEqual(module.delivery_counts(log), (0, 1))


if __name__ == "__main__":
    unittest.main()
