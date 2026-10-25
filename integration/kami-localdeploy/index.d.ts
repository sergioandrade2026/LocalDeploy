import { EventEmitter } from 'node:events';
export interface Project { id: number; name: string; local_path: string; default_branch: string; repo_url: string; }
export interface Commit { sha: string; short_sha: string; author: string; date: string; subject: string; location: 'local' | 'branch' | 'main'; }
export interface Build { id: number; project_id: number; commit_sha: string; status: string; environment: string; endpoint: string | null; workspace_path: string; log_path: string; process_id: number | null; }
export interface Job { id: number; project_id: number | null; kind: string; status: 'running' | 'cancelling' | 'succeeded' | 'failed' | 'cancelled'; progress: string; result: unknown; error: string | null; }
export interface EngineOptions { executable: string; dataDirectory: string; requestTimeoutMs?: number; }
export interface Preview { project: Project; build: Build; endpoint: string | null; kind: 'web' | 'desktop'; }
export class LocalDeployError extends Error { code: number; job?: Job; }
export class KamiLocalDeploy extends EventEmitter {
  info?: { name: string; version: string; protocol_version: number; capabilities: string[]; data_directory: string };
  static start(options: EngineOptions): Promise<KamiLocalDeploy>;
  request<T = unknown>(method: string, params?: Record<string, unknown>): Promise<T>;
  attach(workspace: string, options?: {branch?: string; repoUrl?: string}): Promise<Project>;
  listCommits(projectId: number, limit?: number): Promise<Commit[]>;
  waitJob<T = unknown>(jobId: number, options?: {signal?: AbortSignal; timeoutMs?: number}): Promise<T>;
  testCommit(options: {workspace: string; branch?: string; commit?: string; trusted: boolean; runTests?: boolean; signal?: AbortSignal}): Promise<Preview>;
  removePreview(projectId: number, buildId: number): Promise<{removed: boolean}>;
  close(): Promise<void>;
  on(event: 'job', listener: (job: Job) => void): this;
  on(event: 'event', listener: (event: {type: string; job: Job}) => void): this;
  on(event: 'diagnostic', listener: (text: string) => void): this;
  on(event: string, listener: (...args: any[]) => void): this;
}
export interface WorkspaceController {
  project: Project;
  snapshot(): Promise<{projectName: string; commits: Commit[]; builds: Build[]}>;
  testCommit(commit: string, options?: {runTests?: boolean; signal?: AbortSignal}): Promise<Preview>;
  openPreview(buildId: number): Promise<Build>;
  removePreview(buildId: number): Promise<{removed: boolean}>;
  showLogs(buildId: number): Promise<string>;
  cancelJob(jobId: number): Promise<Job>;
  onJob(listener: (job: Job) => void): () => void;
}
export function createWorkspaceController(client: KamiLocalDeploy, options: {
  workspace: string; branch?: string; isTrusted?: () => boolean;
  openPreview?: (url: string, build: Build) => void | Promise<void>;
  showLogs?: (text: string, buildId: number) => void | Promise<void>;
}): Promise<WorkspaceController>;
