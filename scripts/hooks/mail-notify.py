"""【Hooks】【邮箱通知】接收 Stop 事件 JSON，邮件诊断只写私有日志，不影响终端或主任务。"""

import argparse
from datetime import datetime
import json
import os
from pathlib import Path
import subprocess
import sys


SEND_TIMEOUT_SECONDS = 45


def record_delivery(status: str, message: str, **details) -> None:
    """【Hooks】【通知日志】status 为 sent 或 failed，message 和 details 为诊断内容；追加私有日志，返回无值"""
    try:
        state = Path(os.environ.get("XDG_STATE_HOME") or Path.home() / ".local/state")
        path = Path(
            os.environ.get("SAI_MAIL_LOG") or state / "sai/mail-notify.log"
        ).expanduser()
        path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        entry = {
            "time": datetime.now().astimezone().isoformat(timespec="seconds"),
            "level": "info" if status == "sent" else "error",
            "status": status,
            "message": f"【Hooks】【邮箱通知】{message}",
            **details,
        }
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
        with os.fdopen(descriptor, "a", encoding="utf-8") as output:
            output.write(json.dumps(entry, ensure_ascii=False) + "\n")
    except Exception:
        # 1. 【Hooks】【终端保护】日志不可写时也不回退到终端，通知不是主任务的失败条件
        return


def main() -> int:
    """【Hooks】【发送通知】从命令行和标准输入读取邮件动作，实际结果写日志；非关键通知始终返回零退出码"""
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--mailer",
        default=os.environ.get(
            "SAI_MAIL_SCRIPT",
            str(Path.home() / ".claude/skills/qq-mail/scripts/send_mail.py"),
        ),
    )
    arguments = parser.parse_args()
    try:
        # 1. 【Hooks】【协议检查】限定输入大小，只处理具备会话和轮次标识的 Stop 事件
        payload = sys.stdin.read(1_048_577)
        if len(payload) > 1_048_576:
            raise ValueError("Hook input exceeds 1 MiB")
        event = json.loads(payload)
        if not isinstance(event, dict):
            raise ValueError("Hook input must be a JSON object")
        if event.get("hook_event_name") != "Stop":
            return 0
        session = str(event.get("session_id", ""))
        turn = str(event.get("turn_id", ""))
        if not session or not turn:
            return 0
        mailer = Path(arguments.mailer).expanduser()
        if not mailer.is_file():
            raise FileNotFoundError("Local mail script is unavailable")
        body = "\n".join(
            [
                "Sai is preparing to complete a turn.",
                f"Session: {session}",
                f"Turn: {turn}",
                f"Workspace: {event.get('cwd', '')}",
                "",
                str(event.get("last_assistant_message", ""))[:4096],
            ]
        )
        # 2. 【Hooks】【输出隔离】捕获子进程两路输出，避免异常堆栈和终端控制字符进入 TUI
        result = subprocess.run(
            [
                sys.executable,
                str(mailer),
                "--subject",
                "Sai turn notification",
                "-m",
                body,
            ],
            capture_output=True,
            text=True,
            errors="replace",
            timeout=SEND_TIMEOUT_SECONDS,
            check=False,
        )
        # 3. 【Hooks】【结果记录】实际发送结果只进入日志，失败不触发 Sai 后台错误打印
        record_delivery(
            "sent" if result.returncode == 0 else "failed",
            "邮件脚本执行成功" if result.returncode == 0 else "邮件脚本执行失败",
            session_id=session,
            turn_id=turn,
            exit_code=result.returncode,
            stdout=result.stdout[-2048:],
            stderr=result.stderr[-8192:],
        )
    except subprocess.TimeoutExpired:
        record_delivery("failed", "邮件通知超时", timeout_seconds=SEND_TIMEOUT_SECONDS)
    except Exception as error:
        record_delivery("failed", str(error), error_type=type(error).__name__)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
