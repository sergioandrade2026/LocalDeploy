import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { EventEmitter } from 'node:events';

export class LocalDeployError extends Error {
  constructor(message, code = -32000, job) {
    super(message); this.name = 'LocalDeployError'; this.code = code; this.job = job;
  }
}

/** Run this adapter in the IDE host (Electron main, extension host or Node).
 * It never opens a browser: the IDE decides how to display the returned URL.
 */
export class KamiLocalDeploy extends EventEmitter {
  #process; #pending = new Map(); #next = 1; #closed = false;
  constructor({ executable, dataDirectory, requestTimeoutMs = 30000 }) {
    super();
    if (!executable || !dataDirectory) throw new TypeError('executable and dataDirectory are required');
    this.requestTimeoutMs = requestTimeoutMs;
    this.#process = spawn(executable, ['--data-dir', dataDirectory], {
      windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'], shell: false,
    });
    const lines = createInterface({ input: this.#process.stdout });
    lines.on('line', line => {
      let value;
      try { value = JSON.parse(line); }
      catch {
        const error = new LocalDeployError('Engine wrote an invalid protocol message', -32700);
        this.#fail(error); this.emit('protocolError', error); this.#process.kill(); return;
      }
      if (value.method === 'localdeploy/event') {
        this.emit('event', value.params);
        if (value.params?.job) this.emit('job', value.params.job);
        return;
      }
      const pending = this.#pending.get(value.id);
      if (!pending) return;
      clearTimeout(pending.timer); this.#pending.delete(value.id);
      if (value.error) pending.reject(new LocalDeployError(value.error.message, value.error.code));
      else pending.resolve(value.result);
    });
    this.#process.stderr.on('data', data => this.emit('diagnostic', data.toString()));
    this.#process.on('error', error => { this.#closed = true; this.#fail(error); });
    this.#process.on('exit', (code, signal) => {
      this.#closed = true;
      this.#fail(new LocalDeployError(`Engine exited (${code ?? signal})`));
      this.emit('exit', { code, signal });
    });
    this.#process.stdin.on('error', error => this.#fail(error));
  }
  static async start(options) {
    const client = new KamiLocalDeploy(options);
    try {
      client.info = await client.request('initialize');
      if (client.info.protocol_version !== 1) throw new LocalDeployError('Unsupported engine protocol');
      return client;
    } catch (error) { client.#process.kill(); throw error; }
  }
  #fail(error) {
    for (const item of this.#pending.values()) { clearTimeout(item.timer); item.reject(error); }
    this.#pending.clear();
  }
  request(method, params = {}) {
    if (this.#closed) return Promise.reject(new LocalDeployError('Engine is closed'));
    const id = this.#next++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.#pending.delete(id);
        reject(new LocalDeployError(`Request timed out: ${method}`));
      }, this.requestTimeoutMs);
      this.#pending.set(id, { resolve, reject, timer });
      this.#process.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n', error => {
        if (error) { clearTimeout(timer); this.#pending.delete(id); reject(error); }
      });
    });
  }
  attach(workspace, { branch = 'main', repoUrl } = {}) {
    return this.request('projects.attach', { path: workspace, branch, ...(repoUrl ? { repo_url: repoUrl } : {}) });
  }
  listCommits(projectId, limit = 50) {
    return this.request('commits.list', { project_id: projectId, limit });
  }
  async waitJob(jobId, { signal, timeoutMs = 35 * 60 * 1000 } = {}) {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      if (signal?.aborted) {
        await this.request('jobs.cancel', { job_id: jobId }).catch(() => {});
        const error = new Error('Test cancelled'); error.name = 'AbortError'; throw error;
      }
      const job = await this.request('jobs.get', { job_id: jobId });
      if (job.status === 'succeeded') return job.result;
      if (job.status === 'failed' || job.status === 'cancelled') {
        throw new LocalDeployError(job.error || `Job ${job.status}`, -32000, job);
      }
      if (Date.now() > deadline) throw new LocalDeployError('Job wait timed out; query jobs.get before retrying', -32000, job);
      await new Promise(resolve => setTimeout(resolve, 300));
    }
  }
  async testCommit({ workspace, branch = 'main', commit = 'HEAD', trusted = false, runTests = true, signal }) {
    if (!trusted) throw new LocalDeployError('The IDE must establish project trust before testing', -32602);
    if (signal?.aborted) { const error = new Error('Test cancelled'); error.name = 'AbortError'; throw error; }
    const project = await this.attach(workspace, { branch });
    const { job_id } = await this.request('builds.prepare', {
      project_id: project.id, commit, trusted, run_tests: runTests,
    });
    const build = await this.waitJob(job_id, { signal });
    if (signal?.aborted) { const error = new Error('Test cancelled'); error.name = 'AbortError'; throw error; }
    const opening = await this.request('previews.open', { project_id: project.id, build_id: build.id });
    const started = await this.waitJob(opening.job_id);
    return { project, build: started, endpoint: started.endpoint, kind: started.endpoint ? 'web' : 'desktop' };
  }
  async removePreview(projectId, buildId) {
    const { job_id } = await this.request('previews.remove', { project_id: projectId, build_id: buildId });
    return this.waitJob(job_id);
  }
  async close() {
    if (this.#closed) return;
    // Busy engines reject shutdown. The host should cancel/wait for jobs first.
    await this.request('shutdown');
    this.#process.stdin.end();
  }
}

/** Host facade matching examples/panel-bridge.mjs. Bind these methods to the
 * IDE's own IPC; do not expose arbitrary request() to an untrusted webview.
 */
export async function createWorkspaceController(client, {
  workspace, branch = 'main', isTrusted = () => false,
  openPreview = async () => {}, showLogs = async () => {},
}) {
  const project = await client.attach(workspace, { branch });
  async function present(build) {
    if (build.endpoint) await openPreview(build.endpoint, build);
    return build;
  }
  return {
    project,
    async snapshot() {
      const [commits, builds] = await Promise.all([
        client.listCommits(project.id),
        client.request('builds.list', { project_id: project.id }),
      ]);
      return { projectName: project.name, commits, builds };
    },
    async testCommit(commit, options = {}) {
      const preview = await client.testCommit({ ...options, workspace, branch, commit, trusted: !!isTrusted() });
      await present(preview.build); return preview;
    },
    async openPreview(buildId) {
      const { job_id } = await client.request('previews.open', { project_id: project.id, build_id: buildId });
      return present(await client.waitJob(job_id));
    },
    removePreview(buildId) { return client.removePreview(project.id, buildId); },
    async showLogs(buildId) {
      const result = await client.request('logs.read', { project_id: project.id, build_id: buildId });
      await showLogs(result.text, buildId); return result.text;
    },
    cancelJob(jobId) { return client.request('jobs.cancel', { job_id: jobId }); },
    onJob(listener) {
      const filtered = job => { if (job.project_id === project.id) listener(job); };
      client.on('job', filtered); return () => client.off('job', filtered);
    },
  };
}
