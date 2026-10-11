import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { fetchComputerUseStatus, installComputerUseDriver } from "../../../api/computer-use";

const STATUS_KEY = ["computer-use-status"] as const;

type UseComputerUseDriverOptions = {
  /** 安装完成后把新路径写回草稿 */
  onInstalledPath?: (path: string) => void;
};

/**
 * 【桌面操作】【驱动状态】查询本机驱动并在确认后发起安装。
 *
 * @param options 安装成功后的路径回调
 * @returns 状态查询与安装变更
 */
export function useComputerUseDriver(options: UseComputerUseDriverOptions = {}) {
  const queryClient = useQueryClient();
  const status = useQuery({
    queryKey: STATUS_KEY,
    queryFn: fetchComputerUseStatus,
    staleTime: 10_000
  });
  const install = useMutation({
    mutationFn: installComputerUseDriver,
    onSuccess: async (result) => {
      options.onInstalledPath?.(result.path);
      await queryClient.invalidateQueries({ queryKey: STATUS_KEY });
    }
  });
  return { status, install };
}
