import { useQuery } from "@tanstack/react-query";
import { Plus, Server, Settings2, SquareTerminal } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { createPortal } from "react-dom";
import { useNavigate } from "react-router-dom";
import { api } from "../../api/client";
import { Button } from "../../shared/ui/button/button";
import { sshHostAddress } from "../settings/ssh/ssh-host-form-state";
import { useI18n } from "../i18n/use-i18n";
import "./ssh-target-picker.css";

type SshTargetPickerProps = {
  onCreateLocal: () => void;
  onCreateSsh: (hostId: string) => void;
};

const MENU_WIDTH = 208;
const MENU_MAX_HEIGHT = 256;
const VIEWPORT_PADDING = 12;
const MENU_GAP = 6;

/**
 * 新建终端按钮，可选择本地 Shell 或某台 SSH 主机。
 *
 * 菜单始终展开，未配置主机时给出前往 SSH 设置的入口：
 * 直接创建本地终端会让 SSH 能力完全不可见，用户无从得知它存在。
 *
 * 菜单渲染到 Portal：触发器位于底部终端标签栏，默认向上展开，
 * 避免落到 xterm 区域被视觉淹没；就地绝对定位会被 overflow 裁剪。
 *
 * @param props 本地与 SSH 创建回调
 * @returns 新建终端按钮
 */
export function SshTargetPicker(props: SshTargetPickerProps) {
  const { t } = useI18n();
  const navigate = useNavigate();
  const rootRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [menuStyle, setMenuStyle] = useState<CSSProperties>({
    position: "fixed",
    top: 0,
    left: 0,
    width: MENU_WIDTH
  });
  const hosts = useQuery({ queryKey: ["ssh-hosts"], queryFn: api.ssh.list, staleTime: 30_000 });
  const items = hosts.data?.hosts ?? [];

  useLayoutEffect(() => {
    if (!open) return;

    /** 底部终端触发器优先向上展开，空间不够再落到下方。 */
    const updatePosition = () => {
      const rect = rootRef.current?.getBoundingClientRect();
      if (!rect) return;
      const width = Math.min(MENU_WIDTH, Math.max(0, window.innerWidth - VIEWPORT_PADDING * 2));
      const preferredLeft = rect.right - width;
      const left = Math.max(
        VIEWPORT_PADDING,
        Math.min(preferredLeft, window.innerWidth - width - VIEWPORT_PADDING)
      );
      const spaceAbove = rect.top - MENU_GAP - VIEWPORT_PADDING;
      const spaceBelow = window.innerHeight - rect.bottom - MENU_GAP - VIEWPORT_PADDING;
      // 底部栏默认向上；上方过窄且下方更宽裕时才向下
      const nextStyle: CSSProperties =
        spaceAbove >= 120 || spaceAbove >= spaceBelow
          ? {
              position: "fixed",
              left,
              width,
              bottom: window.innerHeight - rect.top + MENU_GAP,
              maxHeight: Math.max(0, Math.min(MENU_MAX_HEIGHT, spaceAbove))
            }
          : {
              position: "fixed",
              left,
              width,
              top: rect.bottom + MENU_GAP,
              maxHeight: Math.max(0, Math.min(MENU_MAX_HEIGHT, spaceBelow))
            };
      setMenuStyle(nextStyle);
    };

    updatePosition();
    window.addEventListener("resize", updatePosition);
    window.addEventListener("scroll", updatePosition, true);
    return () => {
      window.removeEventListener("resize", updatePosition);
      window.removeEventListener("scroll", updatePosition, true);
    };
  }, [open]);

  useEffect(() => {
    if (!open) return;
    /** 在触发器和 Portal 菜单外按下指针时收起。 */
    const closeOutside = (event: PointerEvent) => {
      const target = event.target as Node;
      const insideTrigger = Boolean(rootRef.current?.contains(target));
      const insideMenu = Boolean(menuRef.current?.contains(target));
      if (!insideTrigger && !insideMenu) {
        setOpen(false);
      }
    };
    /** 按下 Esc 时收起，保持与其他浮层一致。 */
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    // 推迟绑定，避免同一点击手势里的冒泡/延迟 pointer 事件立刻关掉菜单
    const timer = window.setTimeout(() => {
      document.addEventListener("pointerdown", closeOutside);
      document.addEventListener("keydown", closeOnEscape);
    }, 0);
    return () => {
      window.clearTimeout(timer);
      document.removeEventListener("pointerdown", closeOutside);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [open]);

  return (
    <div className="ssh-target-picker" ref={rootRef}>
      <Button
        className="bottom-terminal-new"
        onClick={(event) => {
          setOpen((value) => !value);
        }}
        aria-label={t("New terminal", "新建终端")}
        aria-expanded={open}
        aria-haspopup="menu"
        title={t("New terminal", "新建终端")}
      >
        <Plus size={14} />
      </Button>
      {open && createPortal(
        <div ref={menuRef} className="ssh-target-menu" role="menu" style={menuStyle}>
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              setOpen(false);
              props.onCreateLocal();
            }}
          >
            <SquareTerminal size={13} />
            <span>{t("Local shell", "本地 Shell")}</span>
          </button>
          <div className="ssh-target-separator" />
          {items.map((host) => (
            <button
              type="button"
              role="menuitem"
              key={host.id}
              onClick={() => {
                setOpen(false);
                props.onCreateSsh(host.id);
              }}
            >
              <Server size={13} />
              <span>{host.label}</span>
              <small>{sshHostAddress(host)}</small>
            </button>
          ))}
          {items.length === 0 && (
            <button
              type="button"
              role="menuitem"
              className="ssh-target-configure"
              onClick={() => {
                setOpen(false);
                navigate("/settings/ssh");
              }}
            >
              <Settings2 size={13} />
              <span>{t("Add SSH host", "添加 SSH 主机")}</span>
            </button>
          )}
        </div>,
        document.body
      )}
    </div>
  );
}
