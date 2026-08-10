/**
 * TypeScript mirror of the proposed Extension API 0.9.0 pull-request WIT.
 *
 * This is the Deno / JS ABI shape only. The authoritative contract remains the
 * WIT in `crates/extension_api/wit/since_v0.9.0`. Do not introduce a separate
 * V8 mouthpiece that executes arbitrary WASM.
 */

export interface ParsedGitRemote {
  owner: string;
  repo: string;
}

export interface PullRequestQuery {
  query?: string | null;
  limit?: number | null;
}

export interface PullRequestProviderMetadata {
  id: string;
  label: string;
  supportsReviewComments: boolean;
}

export interface PullRequestSummary {
  number: number;
  title: string;
  url: string;
  author?: string | null;
  isDraft: boolean;
}

export interface ReviewBatchComment {
  filePath: string;
  startLine: number;
  endLine: number;
  body: string;
  excerpt?: string | null;
}

export interface ReviewBatch {
  comments: ReviewBatchComment[];
}

export interface PullRequestReviewComment {
  id: string;
  author: string;
  body: string;
}

export interface PullRequestReviewThread {
  id: string;
  filePath: string;
  startLine: number;
  endLine: number;
  excerpt?: string | null;
  isResolved: boolean;
  comments: PullRequestReviewComment[];
}

export interface PullRequestDetail {
  summary: PullRequestSummary;
  files: string[];
  threads: PullRequestReviewThread[];
}

export interface Extension {
  activate?(): void | Promise<void>;

  languageServerCommand?(id: string, worktree: unknown): Promise<unknown>;
  contextServerCommand?(id: string, project: unknown): Promise<unknown>;

  pullRequestProviderMetadata?(
    id: string,
  ): Promise<PullRequestProviderMetadata>;
  listPullRequests?(
    id: string,
    remote: ParsedGitRemote,
    query: PullRequestQuery,
  ): Promise<PullRequestSummary[]>;
  getPullRequest?(
    id: string,
    remote: ParsedGitRemote,
    number: number,
  ): Promise<PullRequestDetail>;
  postReviewComments?(
    id: string,
    remote: ParsedGitRemote,
    number: number,
    batch: ReviewBatch,
  ): Promise<void>;
  resolveReviewThread?(id: string, threadId: string): Promise<void>;
}
