/** Framework-independent renderer component. No Node APIs or subprocess access.
 * Listen for its events in the IDE's trusted bridge. Repository text is always
 * assigned through textContent, never interpreted as HTML.
 */
export class KamiDeploymentsPanel extends HTMLElement {
  #data = { commits: [], builds: [], projectName: '', productionCommit: null };
  #job = null; #message = '';
  constructor() {
    super(); this.attachShadow({ mode: 'open' });
    this.shadowRoot.innerHTML = `<style>
      :host{display:block;color:var(--kami-text,#222);background:var(--kami-background,#fff);font:14px/1.5 var(--kami-font,'Segoe UI',sans-serif)}[hidden]{display:none!important}
      header{display:flex;justify-content:space-between;align-items:center;padding:16px 0}h2{font-size:22px;margin:0}p{margin:4px 0;color:var(--kami-muted,#737373)}
      button{font:inherit;color:inherit;cursor:pointer;background:transparent;border:1px solid var(--kami-border,#ddd);border-radius:7px;padding:7px 12px}button:disabled{opacity:.5;cursor:wait}button:focus-visible{outline:2px solid #4297ff;outline-offset:2px}
      .list{border:1px solid var(--kami-border,#e4e4e4);border-radius:9px;overflow:hidden}.row{display:grid;grid-template-columns:minmax(160px,1fr) auto auto;align-items:center;gap:16px;padding:14px 16px;border-bottom:1px solid var(--kami-border,#eee)}.row:last-child{border-bottom:0}.subject{font-weight:600;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.meta{font-size:12px;color:var(--kami-muted,#777)}.actions{display:flex;align-items:center;gap:8px}.badge{border-radius:99px;min-width:84px}.status{font-size:13px;white-space:nowrap}.ready{color:#14866a}.failed{color:#c33838}.empty{padding:32px;text-align:center}.progress{padding:12px 0;display:flex;justify-content:space-between;align-items:center}.message{padding:12px 0;white-space:pre-wrap;overflow-wrap:anywhere}@media(max-width:650px){.row{grid-template-columns:1fr auto}.status{grid-column:1}.actions{grid-column:2;grid-row:1/3}.subject{white-space:normal}}
    </style><header><div><h2>Deployments</h2><p id="project"></p></div><button id="refresh">Atualizar</button></header><div class="progress" hidden><span role="status"></span><button>Cancelar</button></div><div class="message" role="status" hidden></div><div class="list"></div>`;
    this.shadowRoot.querySelector('#refresh').onclick = () => this.#emit('refresh');
    this.shadowRoot.querySelector('.progress button').onclick = () => this.#emit('cancel-job', { job: this.#job });
  }
  setData(data) { this.#data = { ...this.#data, ...data }; this.#render(); }
  setJob(job) { this.#job = job; this.#render(); }
  setMessage(message = '') { this.#message = String(message); this.#render(); }
  #emit(type, detail = {}) { this.dispatchEvent(new CustomEvent(type, { detail, bubbles: true, composed: true })); }
  #render() {
    const root = this.shadowRoot;
    const busy = this.#job && ['running', 'cancelling'].includes(this.#job.status);
    root.querySelector('#project').textContent = this.#data.projectName;
    root.querySelector('.progress').hidden = !busy;
    root.querySelector('.progress span').textContent = busy ? this.#job.progress || 'Preparando teste…' : '';
    root.querySelector('.progress button').disabled = this.#job?.kind !== 'prepare' || this.#job?.status === 'cancelling';
    root.querySelector('.message').hidden = !this.#message;
    root.querySelector('.message').textContent = this.#message;
    const list = root.querySelector('.list'); list.replaceChildren();
    if (!this.#data.commits.length) {
      const empty = document.createElement('div'); empty.className = 'empty'; empty.textContent = 'Nenhum commit para exibir. Abra um repositório na IDE.'; list.append(empty); return;
    }
    for (const commit of this.#data.commits) {
      const build = this.#data.builds.find(b => b.commit_sha === commit.sha && b.environment === 'preview');
      const row = document.createElement('div'); row.className = 'row';
      const content = document.createElement('div');
      const subject = document.createElement('div'); subject.className = 'subject'; subject.textContent = commit.subject;
      const meta = document.createElement('div'); meta.className = 'meta'; meta.textContent = `${commit.short_sha} · ${commit.author} · ${commit.date}`;
      content.append(subject, meta);
      const status = document.createElement('div'); status.className = 'status';
      status.textContent = build ? ({ready:build.endpoint ? '● Ativo' : '● Aprovado',failed:'● Erro',cancelled:'Cancelado',stopped:'Parado',expired:'Removido'}[build.status] || 'Preparando') : 'Não testado';
      if (build?.status === 'ready') status.classList.add('ready');
      if (build?.status === 'failed') status.classList.add('failed');
      const actions = document.createElement('div'); actions.className = 'actions';
      const test = document.createElement('button'); test.className = 'badge'; test.disabled = !!busy;
      test.textContent = this.#data.productionCommit === commit.sha ? 'Produção' : ({local:'Local',branch:'Preview',main:'Main'}[commit.location] || 'Preview');
      test.title = build?.endpoint ? 'Abrir teste local deste commit' : 'Preparar e abrir teste deste commit';
      test.onclick = () => this.#emit(build?.status === 'ready' ? 'open-preview' : 'test-commit', { commit, build });
      actions.append(test);
      if (build) {
        const logs = document.createElement('button'); logs.textContent = 'Logs'; logs.onclick = () => this.#emit('show-logs', { commit, build }); actions.append(logs);
        if (build.status !== 'expired') {
          const remove = document.createElement('button'); remove.textContent = '×'; remove.title = 'Remover arquivos do teste'; remove.setAttribute('aria-label','Remover teste'); remove.disabled = !!busy; remove.onclick = () => this.#emit('remove-preview', { commit, build }); actions.append(remove);
        }
      }
      row.append(content, status, actions); list.append(row);
    }
  }
}
if (!customElements.get('kami-deployments')) customElements.define('kami-deployments', KamiDeploymentsPanel);
