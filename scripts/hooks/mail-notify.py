"""【Hooks】【邮箱通知】接收 Stop 事件 JSON，调用本地邮箱脚本，不保存凭据。"""

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys


def main() -> int:
    """【Hooks】【发送通知】参数来自命令行与标准输入，返回发送结果对应的退出码。"""
    parser = argparse.ArgumentParser()
    parser.add_argument("--mailer", default=os.environ.get(
        "SAI_MAIL_SCRIPT", str(Path.home() / ".claude/skills/qq-mail/scripts/send_mail.py")))
    arguments = parser.parse_args()
    payload = sys.stdin.read(1_048_577)
    if len(payload) > 1_048_576:
        print("Hook input exceeds 1 MiB", file=sys.stderr)
        return 1
    event = json.loads(payload)
    if event.get("hook_event_name") != "Stop":
        return 0
    session = str(event.get("session_id", ""))
    turn = str(event.get("turn_id", ""))
    if not session or not turn:
        return 0
    mailer = Path(arguments.mailer).expanduser()
    if not mailer.is_file():
        print("Local mail script is unavailable", file=sys.stderr)
        return 1
    body = "\n".join([
        "Sai is preparing to complete a turn.",
        f"Session: {session}", f"Turn: {turn}",
        f"Workspace: {event.get('cwd', '')}",
        "", str(event.get("last_assistant_message", ""))[:4096],
    ])
    # 1. 【Hooks】【输出隔离】邮箱脚本诊断进入标准错误，不污染钩子 JSON 协议
    result = subprocess.run(
        [sys.executable, str(mailer), "--subject", "Sai turn notification", "-m", body],
        stdout=sys.stderr, stderr=sys.stderr, timeout=45, check=False,
    )
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
