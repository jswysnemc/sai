"""【终端】【Windows 路径着色】从真实模型参数流验证路径、行末字符和逻辑换行。"""

import json
import unittest

from mock_tool_model import MockToolModel
from terminal import TerminalSession


COMMANDS = [
    r"git -C D:\Code\Systemui-dev log -n 5 --oneline src/com/android/systemui/keyguard/ui/viewmodel/DeviceEntryIconViewModel",
    r"git -C D:\Code\Systemui-dev log -n 5 --oneline src/com/android/systemui/keyguard/ui/viewmodel/OffToLockscreenTransition",
]
SCRIPT = r'''$extractDir = "D:\work\11月 \11.4-DEPLOYC-16124-锁屏点想帮帮无反应\attachments\extracted"
$selected = @()
$folded = @()

foreach ($f in $allFiles) {
    # 排除 dropbox 碎片与 logcats 分卷
    $rel = $f.FullName.Substring($extractDir.Length + 1)'''


class WindowsPathColoringTests(unittest.TestCase):
    """使用执行器已有的 PowerShell 参数，从 Linux 终端验证 Windows 语法显示。"""

    def assert_command(self, command, columns):
        """参数为原始命令和终端列数；验证预览原文、颜色及缩放，返回无。"""
        with MockToolModel("run_command") as model:
            config = model.config()
            config["display"] = {"fullscreen": False, "fold_head_lines": 10, "fold_tail_lines": 10}
            with TerminalSession(config, columns=columns, rows=40,
                                 arguments=["--lang", "en-US"]) as terminal:
                terminal.send(b"Inspect Windows command preview\r")
                terminal.wait_for(model.ready.is_set)
                # 1. 【终端】【参数流】保留执行器的语法提示，不结束调用或执行测试命令
                arguments = json.dumps({"_sai_command_shell": "pwsh", "command": command})
                model.fragments.put(arguments[:-1])
                terminal.wait_for(lambda: "Running $ " in terminal.text())
                terminal.pump(.2)
                self.assert_source_and_colors(terminal, command)
                model.fragments.put("}")
                terminal.pump(.2)
                self.assert_source_and_colors(terminal, command)
                # 2. 【终端】【尺寸边界】缩放后末尾引号和文件名仍完整保留
                terminal.resize(columns - 9, 40)
                terminal.pump(.3)
                self.assert_source_and_colors(terminal, command)

    def assert_source_and_colors(self, terminal, command):
        """参数为终端与源码；检查可见字符和实际终端单元格颜色，返回无。"""
        rows = terminal.screen.display
        start = next(index for index, row in enumerate(rows) if "Running $ " in row)
        end = next(index for index in range(start + 1, len(rows)) if "Waiting to run" in rows[index])
        body = rows[start].split("Running $ ", 1)[1] + "".join(rows[start + 1:end])
        self.assertEqual("".join(body.split()), "".join(command.split()))
        if command in COMMANDS:
            row = rows[start]
            path_column = row.index(r"D:\Code\Systemui-dev")
            colors = [terminal.screen.buffer[start][column].fg
                      for column in range(path_column, path_column + len(r"D:\Code\Systemui-dev"))]
            # 路径分隔符保持普通颜色，不能单独变成字符串转义色
            self.assertNotIn("a6d6a0", colors)
            log = row.index("log")
            self.assertEqual(terminal.screen.buffer[start][log].fg, "d7d7ff")
        else:
            comment = next(index for index, row in enumerate(rows) if "# 排除" in row)
            last = next(index for index, row in enumerate(rows) if "$rel =" in row)
            self.assertEqual(terminal.screen.buffer[comment][rows[comment].index("#")].fg, "6e6a86")
            self.assertEqual(terminal.screen.buffer[last][rows[last].index("rel")].fg, "d7d7ff")
            self.assertEqual(terminal.screen.buffer[last][rows[last].index("1)")].fg, "f6c177")

    def test_git_commands_keep_windows_paths_and_filename_tails(self):
        """无参数；用户提供的两条命令在行末边界及缩放后保留原文和颜色，返回无。"""
        for command in COMMANDS:
            with self.subTest(command=command):
                self.assert_command(command, 131)

    def test_powershell_script_keeps_quote_and_colors_after_comment(self):
        """无参数；真实中文路径和注释后代码在流式阶段与完成阶段保持一致，返回无。"""
        self.assert_command(SCRIPT, 130)


if __name__ == "__main__":
    unittest.main()
