mod desktop;
mod sound;

use crate::config::AppConfig;
use crate::paths::SaiPaths;
use sai_plugin_runtime::{Notification, PresentationSurface, ReplyPresentation, ReplyStatus};

/// 【答复通知】【TUI 入口】计算一次通知策略并把成功结果交给平台投递。
/// @param paths 可信插件目录；config 为当前配置；status 为宿主确定的结束状态
/// @returns 无；策略错误不影响对话结果，CLI 单次命令与子任务不调用此入口
pub(crate) async fn reply_ended(paths: &SaiPaths, config: &AppConfig, status: ReplyStatus) {
    let event = ReplyPresentation {
        surface: PresentationSurface::Tui,
        status,
        locale: crate::i18n::locale().code().to_string(),
    };
    if let Ok(plan) = crate::plugins::notification_plan(config, paths, event).await {
        deliver(plan.notifications);
    }
}

/// 【通知投递】【后台投递】按已校验的数据发送系统通知与提示音，不包含业务选择规则。
/// @param notifications 本次纯回调返回的通知
/// @returns 无；系统通知失败不会创建后台命令或自动续聊任务
fn deliver(notifications: Vec<Notification>) {
    if notifications.is_empty() {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("sai-notification-delivery".into())
        .spawn(move || {
            for notification in notifications {
                if notification.desktop {
                    let _ = desktop::send(&notification.title, &notification.body);
                }
                if notification.sound {
                    let _ = sound::play();
                }
            }
        });
}
