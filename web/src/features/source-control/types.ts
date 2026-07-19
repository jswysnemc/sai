import type { GitOperationResponse } from "../../api/contracts";
import type { GitOperationAction, GitOperationOptions } from "../../api/git-contracts";

export type GitOperationUiOptions = GitOperationOptions & {
  confirmTitle?: string;
  confirmDescription?: string;
};

export type RunGitOperation = (
  action: GitOperationAction,
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
