import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, rm, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';
import { KamiLocalDeploy } from '../kami-localdeploy/index.mjs';

const executable = process.env.LOCALDEPLOY_ENGINE || fileURLToPath(new URL(`../../target/debug/localdeploy-engine${process.platform === 'win32' ? '.exe' : ''}`, import.meta.url));

test('IDE adapter: isolated commits, localhost, errors, async jobs and cleanup without desktop', { timeout: 90000 }, async () => {
  const temporary = await mkdtemp(join(tmpdir(), 'kami-localdeploy-'));
  const workspace = join(temporary, 'Projeto com espaços ç');
  await mkdir(workspace);
  const git = (...args) => execFileSync('git', ['-C', workspace, ...args], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();
  git('init', '-b', 'main'); git('config', 'user.name', 'Kami Test'); git('config', 'user.email', 'kami@example.test');
  const fixture = async (text, fail = false) => {
    await writeFile(join(workspace, 'package.json'), JSON.stringify({ name: 'kami-fixture', version: '1.0.0', scripts: {
      build: fail ? 'node -e "process.exit(7)"' : `node -e "require('fs').mkdirSync('dist',{recursive:true});require('fs').writeFileSync('dist/index.html','${text}')"`,
    } }));
    await writeFile(join(workspace, 'package-lock.json'), JSON.stringify({ name:'kami-fixture', version:'1.0.0', lockfileVersion:3, requires:true, packages:{'':{name:'kami-fixture',version:'1.0.0'}} }));
    git('add', '.'); git('commit', '-m', text); return git('rev-parse', 'HEAD');
  };
  const shaA = await fixture('commit-A'); const shaB = await fixture('commit-B');
  const client = await KamiLocalDeploy.start({ executable, dataDirectory: join(temporary, 'engine-data') });
  const previews = []; const events = [];
  client.on('job', job => events.push(job));
  try {
    assert.equal(client.info.protocol_version, 1);
    await assert.rejects(client.request('unknown.method'), error => error.code === -32601);
    await assert.rejects(client.request('projects.attach', { path: 'relative' }), error => error.code === -32602);
    await client.request('settings.write', { backend: 'Nativo' });
    const project = await client.attach(workspace);
    assert.equal((await client.attach(workspace)).id, project.id, 'attach is idempotent');
    assert.equal((await client.listCommits(project.id)).length, 2);
    await assert.rejects(client.testCommit({ workspace, commit: shaA, trusted: false }), error => error.code === -32602);
    for (const [sha, expected] of [[shaA, 'commit-A'], [shaB, 'commit-B']]) {
      const preview = await client.testCommit({ workspace, commit: sha, trusted: true });
      previews.push(preview);
      assert.equal(await (await fetch(preview.endpoint)).text(), expected);
    }
    assert.notEqual(previews[0].endpoint, previews[1].endpoint);
    assert.equal(await (await fetch(previews[0].endpoint)).text(), 'commit-A', 'new commit does not mutate the older preview');
    assert.ok(events.some(event => event.status === 'succeeded'));
    for (const preview of previews.splice(0)) {
      await client.removePreview(preview.project.id, preview.build.id);
      await assert.rejects(access(preview.build.workspace_path));
    }
    const failure = await fixture('failing-build', true);
    const submitted = await client.request('builds.prepare', { project_id: project.id, commit: failure, trusted: true });
    assert.ok(Array.isArray(await client.request('projects.list')), 'engine remains responsive while building');
    await assert.rejects(client.waitJob(submitted.job_id), /código|code|terminou/);
    const builds = await client.request('builds.list', { project_id: project.id });
    assert.equal(builds[0].status, 'failed');
    const log = await client.request('logs.read', { project_id: project.id, build_id: builds[0].id });
    assert.match(log.text, /build/);
    await writeFile(join(workspace, 'package.json'), JSON.stringify({ name:'kami-fixture',version:'1.0.0',scripts:{build:'node -e "setTimeout(()=>{},30000)"'} }));
    git('add', '.'); git('commit', '-m', 'long build for cancellation');
    const slow = await client.request('builds.prepare', { project_id: project.id, commit:git('rev-parse','HEAD'), trusted:true });
    const deadline = Date.now() + 15000;
    while (Date.now() < deadline) {
      const job = await client.request('jobs.get', { job_id:slow.job_id });
      if (job.progress.includes('run build')) break;
      await new Promise(resolve=>setTimeout(resolve,100));
    }
    await client.request('jobs.cancel', { job_id:slow.job_id });
    await assert.rejects(client.waitJob(slow.job_id), /cancelad|cancelled/);
    assert.equal((await client.request('jobs.get', {job_id:slow.job_id})).status, 'cancelled');
  } finally {
    for (const preview of previews) await client.removePreview(preview.project.id, preview.build.id).catch(() => {});
    await client.close();
    await rm(temporary, { recursive: true, force: true });
  }
});
