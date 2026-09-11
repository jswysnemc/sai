use super::support::FixtureHost;
use sai_plugin_runtime::{Capabilities, InvocationContext, PluginPackage, PluginRuntime};
use serde_json::{json, Value};
use std::sync::Arc;

/// 【玄学迁移测试】【实际源码】读取发布包，可在入口前后注入测试代码
/// @param before 入口前的随机替身；after 入口后的验证注册
/// @returns 保留真实清单和业务模块的完整包
pub(super) fn package(before: &str, after: &str) -> PluginPackage {
    let package = crate::plugins::bundled::packages()
        .unwrap()
        .into_iter()
        .find(|package| package.manifest.id == "xuanxue")
        .expect("missing bundled xuanxue package");
    let mut sources = package.sources().clone();
    let entry = sources.get_mut(&package.manifest.entry).unwrap();
    *entry = format!("{before}\n{entry}\n{after}");
    PluginPackage::new(package.manifest, sources).unwrap()
}

/// 【玄学迁移测试】【零权限加载】使用拒绝外部能力的宿主加载待测包
/// @param package 完整插件源码和资源限制
/// @returns 可独立调用的运行时
pub(super) fn load(package: PluginPackage) -> PluginRuntime {
    PluginRuntime::load(
        package,
        json!({}),
        Capabilities::default(),
        Arc::new(FixtureHost::default()),
    )
    .unwrap()
}

/// 【玄学迁移测试】【固定随机源】注入原版记录的区间和点数，不修改实际业务代码
/// @param draws 每次随机抽取的 [下界, 上界, 结果] 数组
/// @returns 带有消耗检查命令的测试运行时
pub(super) fn fixed_runtime(draws: Value) -> PluginRuntime {
    let text = serde_json::to_string(&draws.to_string()).unwrap();
    let before = format!("local draws = sai.json.decode({text})\n");
    load(package(
        &(before
            + r#"
        local position = 0
        --- 【玄学迁移测试】【随机替身】逐次核对业务请求的区间并返回原版固定选择
        --- @param low integer 下界，单参数调用表示上界
        --- @param high integer|nil 上界
        --- @return integer 原版样本值
        math.random = function(low, high)
            if high == nil then low, high = 1, low end
            position = position + 1
            local draw = assert(draws[position], "unexpected random draw")
            assert(low == draw[1] and high == draw[2], "random bounds changed")
            return draw[3]
        end
        --- 【玄学迁移测试】【消耗检查】验证每个固定随机值恰好消费一次
        --- @return integer 实际随机抽取次数
        local function consumed()
            assert(position == #draws, "unused random draws")
            return position
        end
        sai.register_command({name="fixture_consumed",description="Check random calls",execute=consumed})
    "#),
        "",
    ))
}

/// 【玄学迁移测试】【随机核验】要求测试样本的全部随机值已经使用
/// @param plugin 固定随机源运行时；expected 为预期抽取次数
/// @returns 无；次数或区间不同则测试失败
pub(super) async fn assert_consumed(plugin: &PluginRuntime, expected: usize) {
    assert_eq!(
        plugin
            .call_command("fixture_consumed", "", InvocationContext::default())
            .await
            .unwrap(),
        expected.to_string()
    );
}
