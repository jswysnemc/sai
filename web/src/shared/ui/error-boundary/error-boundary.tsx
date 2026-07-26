import { Component, type ErrorInfo, type ReactNode } from "react";
import { ErrorFallback } from "./error-fallback";
import "./error-boundary.css";

type ErrorBoundaryProps = {
  children: ReactNode;
  /** 出错区域的名称，用于提示文案 */
  label?: string;
  /** 自定义降级内容；不提供时用内置的错误卡片 */
  fallback?: (error: Error, reset: () => void) => ReactNode;
};

type ErrorBoundaryState = {
  error: Error | null;
};

/**
 * 捕获子树渲染异常，避免单个组件出错导致整页空白。
 *
 * React 只支持用类组件实现错误边界，因此这里没有对应的函数式写法。
 * 捕获后展示可重试的错误卡片，其余区域继续可用。
 */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  /**
   * 把渲染异常转成组件状态。
   *
   * @param error 捕获到的异常
   * @returns 携带异常的新状态
   */
  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  /**
   * 记录异常与组件栈，便于排查。
   *
   * @param error 捕获到的异常
   * @param info React 提供的组件栈信息
   * @returns 无
   */
  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error(`[${this.props.label ?? "ui"}] render failed`, error, info.componentStack);
  }

  /**
   * 清除错误状态并重新渲染子树。
   *
   * @returns 无
   */
  reset = () => {
    this.setState({ error: null });
  };

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;
    if (this.props.fallback) return this.props.fallback(error, this.reset);
    return <ErrorFallback error={error} label={this.props.label} onRetry={this.reset} />;
  }
}
