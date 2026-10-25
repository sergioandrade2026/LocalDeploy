// Runs in the IDE renderer, alongside the Web Component. bridge is a restricted
// IPC facade implemented by the IDE host, never a raw shell interface.
import '../kami-localdeploy/panel.mjs';

export async function connectDeploymentsPanel(panel, bridge) {
  const listeners = [];
  const listen = (event, handler) => {
    const guarded = e => Promise.resolve(handler(e.detail)).catch(error => panel.setMessage(error.message));
    panel.addEventListener(event, guarded); listeners.push(() => panel.removeEventListener(event, guarded));
  };
  async function refresh() { panel.setData(await bridge.snapshot()); }
  listen('refresh', refresh);
  listen('test-commit', async ({commit}) => { panel.setMessage(''); await bridge.testCommit(commit.sha); await refresh(); });
  listen('open-preview', async ({build}) => { await bridge.openPreview(build.id); await refresh(); });
  listen('remove-preview', async ({build}) => { await bridge.removePreview(build.id); await refresh(); });
  listen('show-logs', ({build}) => bridge.showLogs(build.id));
  listen('cancel-job', ({job}) => bridge.cancelJob(job.id));
  const unsubscribe = bridge.onJob(job => panel.setJob(job));
  await refresh();
  return () => { listeners.forEach(dispose => dispose()); unsubscribe?.(); };
}
