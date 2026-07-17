import { useState } from "react";
import { WorkspaceLayout } from "./workspace-layout";
import "./coding-page.css";

/** 渲染编程页面并维护当前选中文件。 */
export function CodingPage() {
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  return <WorkspaceLayout selectedFile={selectedFile} onSelectFile={setSelectedFile} onClearFile={() => setSelectedFile(null)} />;
}
