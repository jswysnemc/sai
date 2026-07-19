import type { GitOperationResponse } from "../../api/contracts";
import type { GitOperationOptions } from "../../api/git-contracts";

export type GitOperationUiOptions = GitOperationOptions & {
  confirmTitle?: string;
  confirmDescription?: string;
};

export type RunGitOperation = (
  action: string,
  options?: GitOperationUiOptions
) => Promise<GitOperationResponse | undefined>;

export type GitOutputEntry = {
  id: number;
  action: string;
  ok: boolean;
  message: string;
  stdout: string;
  stderr: string;
  createdAt: number;
};
