import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { DialogProvider } from "../ui/dialog/dialog-provider";

/**
 * 在测试中把节点渲染成静态 HTML，并补齐运行所需的上下文。
 *
 * 组件树深处会用到 useQuery 与 useConfirm，缺少对应 Provider 时渲染直接抛错。
 * i18n 的 Context 自带回落值，因此无需额外包装。
 *
 * @param node 待渲染节点
 * @returns 渲染出的 HTML 字符串
 */
export function renderWithProviders(node: ReactNode): string {
  // 测试中不重试失败请求，否则断言会等到超时
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return renderToStaticMarkup(
    <QueryClientProvider client={client}>
      <DialogProvider>{node}</DialogProvider>
    </QueryClientProvider>
  );
}
