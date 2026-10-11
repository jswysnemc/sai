import { Button } from "../../../shared/ui/button/button";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { Toast, useToast } from "../../../shared/ui/notify/notify";
import { toDisplayError } from "../../../api/api-error";
import { useI18n } from "../../i18n/use-i18n";
import { InlineNotice, StatusBadge, type StatusTone } from "../kit";
import { useComputerUseDriver } from "./use-computer-use-driver";

type ComputerUseDriverPanelProps = {
  /** 当前草稿中的驱动路径 */
  driverPath: string;
  /** 安装完成后回写路径 */
  onInstalledPath: (path: string) => void;
};

/**
 * 【桌面操作】【驱动面板】展示本机探测结果，并在确认后安装官方发布包。
 *
 * @param props 当前路径与安装回调
 * @returns 驱动状态与安装操作
 */
export function ComputerUseDriverPanel({ driverPath, onInstalledPath }: ComputerUseDriverPanelProps) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const { notice, showToast, dismissToast } = useToast();
  const { status, install } = useComputerUseDriver({ onInstalledPath });
  const data = status.data;

  /**
   * 弹出确认框后安装官方驱动。
   *
   * @returns 无返回值
   */
  const requestInstall = async () => {
    const accepted = await confirm({
      title: t("Install cua-driver on this host?", "在本机安装 cua-driver？"),
      description: t(
        `Sai will download the official ${data?.pinned_version ?? "0.34.1"} package into its data directory. This does not enable Computer Use, grant OS permissions, or change PATH.`,
        `Sai 会把官方 ${data?.pinned_version ?? "0.34.1"} 发布包下载到数据目录。不会开启桌面操作、授予系统权限或改写 PATH。`
      ),
      confirmLabel: t("Install", "安装")
    });
    if (!accepted) return;
    try {
      const result = await install.mutateAsync();
      showToast(t(`Installed cua-driver ${result.version}`, `已安装 cua-driver ${result.version}`));
    } catch (error) {
      showToast(toDisplayError(error, "Install failed", "安装失败").message, "error");
    }
  };

  const tone: StatusTone = !data
    ? "neutral"
    : data.installed && data.version_ok
      ? "success"
      : data.installed
        ? "warning"
        : "danger";
  const label = !data
    ? t("Checking", "正在检测")
    : data.installed && data.version_ok
      ? t("Ready", "可用")
      : data.installed
        ? t("Update needed", "需要更新")
        : t("Not installed", "未安装");

  return (
    <div className="computer-use-driver">
      <div className="computer-use-driver-row">
        <StatusBadge tone={tone} dot>{label}</StatusBadge>
        {data?.version && <span className="computer-use-path">{data.version}</span>}
        <div className="computer-use-actions">
          <Button size="small" disabled={status.isFetching} onClick={() => void status.refetch()}>
            {t("Refresh", "重新检测")}
          </Button>
          <Button
            variant="primary"
            size="small"
            disabled={!data?.install_supported || install.isPending}
            onClick={() => void requestInstall()}
          >
            {install.isPending ? t("Installing", "正在安装") : t("Install cua-driver", "安装 cua-driver")}
          </Button>
        </div>
      </div>
      {data?.resolved_path && <p className="computer-use-path">{data.resolved_path}</p>}
      {status.error && (
        <InlineNotice tone="danger">{toDisplayError(status.error, "Driver status error", "驱动状态错误").message}</InlineNotice>
      )}
      {data && !data.install_supported && (
        <InlineNotice tone="warning">{t("Automatic install is unavailable on this platform. Install cua-driver yourself and set the path.", "当前平台不支持自动安装。请自行安装 cua-driver 并填写路径。")}</InlineNotice>
      )}
      {data?.installed && !data.version_ok && (
        <InlineNotice tone="warning">{t(`cua-driver ${data.min_version} or newer is required.`, `需要 cua-driver ${data.min_version} 或更新版本。`)}</InlineNotice>
      )}
      {data?.version_error && <InlineNotice tone="warning">{data.version_error}</InlineNotice>}
      {data?.install_path && driverPath !== data.resolved_path && (
        <InlineNotice tone="info">{t(`Sai install directory: ${data.install_path}`, `Sai 安装目录：${data.install_path}`)}</InlineNotice>
      )}
      <Toast notice={notice} onDismiss={dismissToast} />
    </div>
  );
}
