// 【Windows沙箱测试】【生产模块】驱动只调用非交互 Shell 入口，完整模块仍包含交互终端接口
#[expect(
    dead_code,
    reason = "独立沙箱驱动不调用交互终端接口，主程序另有调用与测试"
)]
#[path = "../../../src/platform/shell_selection.rs"]
pub(crate) mod shell_selection;
