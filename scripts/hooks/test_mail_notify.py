"""【Hooks】【邮箱测试】以临时邮件脚本验证通知协议，不连接 SMTP。"""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


class MailNotifyTests(unittest.TestCase):
    """【Hooks】【通知夹具】验证有效完成事件与无关事件。"""

    def test_stop_payload_and_output_separation(self):
        """无参数；返回无值，Stop 事件发送一次且诊断只进入标准错误。"""
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            output = root / "arguments.json"
            mailer = root / "mailer.py"
            mailer.write_text(f"import sys, json\nfrom pathlib import Path\nPath({str(output)!r}).write_text(json.dumps(sys.argv[1:]))\nprint('SMTP accepted')\n")
            script = Path(__file__).with_name("mail-notify.py")
            event = {"hook_event_name": "Stop", "session_id": "session-test", "turn_id": "turn-test", "cwd": str(root), "last_assistant_message": "completed"}
            result = subprocess.run([sys.executable, str(script), "--mailer", str(mailer)], input=json.dumps(event), text=True, capture_output=True, check=False)
            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "")
            self.assertIn("SMTP accepted", result.stderr)
            self.assertIn("completed", json.loads(output.read_text())[-1])
            output.unlink()
            event["hook_event_name"] = "Interrupt"
            result = subprocess.run([sys.executable, str(script), "--mailer", str(mailer)], input=json.dumps(event), text=True, capture_output=True, check=False)
            self.assertEqual(result.returncode, 0)
            self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
