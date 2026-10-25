import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
const { chromium } = createRequire(import.meta.url)('playwright');

test('embedded panel: actions, safe text, readable rows and cancellation state', async () => {
  const browser = await chromium.launch({ executablePath: process.env.CHROMIUM_BIN || '/usr/bin/chromium', headless: true, args:['--no-sandbox'] });
  try {
    const page = await browser.newPage({ viewport:{width:1100,height:600} });
    await page.setContent('<main style="padding:24px"><kami-deployments></kami-deployments></main>');
    await page.addScriptTag({ type:'module', content:await readFile(new URL('../kami-localdeploy/panel.mjs',import.meta.url),'utf8') });
    await page.evaluate(async () => {
      await customElements.whenDefined('kami-deployments');
      const panel = document.querySelector('kami-deployments');
      window.events = [];
      for (const name of ['test-commit','open-preview','cancel-job']) panel.addEventListener(name,e=>window.events.push({name,detail:e.detail}));
      panel.setData({ projectName:'Kami Archs IDE · projeto de exemplo', commits:[
        {sha:'a',short_sha:'a123456',subject:'Ajustar formulário de cadastro',author:'Desenvolvedor',date:'2026-10-25',location:'local'},
        {sha:'b',short_sha:'b123456',subject:'<img src=x onerror=alert(1)>',author:'Desenvolvedor',date:'2026-10-25',location:'branch'},
        {sha:'c',short_sha:'c123456',subject:'Atualizar navegação principal',author:'Desenvolvedor',date:'2026-10-25',location:'main'},
      ], builds:[{id:1,commit_sha:'a',status:'ready',environment:'preview',endpoint:'http://127.0.0.1:4173'},{id:2,commit_sha:'b',status:'failed',environment:'preview',endpoint:null}] });
    });
    assert.equal(await page.locator('kami-deployments .row').count(),3);
    assert.equal(await page.locator('kami-deployments img').count(),0);
    assert.equal(await page.locator('kami-deployments .progress').isVisible(),false);
    await page.locator('kami-deployments .badge').nth(0).click();
    await page.locator('kami-deployments .badge').nth(1).click();
    assert.deepEqual(await page.evaluate(()=>window.events.map(e=>e.name)),['open-preview','test-commit']);
    await page.evaluate(()=>document.querySelector('kami-deployments').setJob({id:7,kind:'prepare',status:'running',progress:'Executando testes…'}));
    assert.equal(await page.locator('kami-deployments .progress').isVisible(),true);
    assert.equal(await page.locator('kami-deployments .badge').nth(0).isDisabled(),true);
    await page.locator('kami-deployments .progress button').click();
    assert.equal(await page.evaluate(()=>window.events.at(-1).name),'cancel-job');
    await page.evaluate(()=>{
      const panel=document.querySelector('kami-deployments');
      panel.setJob({id:7,kind:'prepare',status:'succeeded'});
      panel.setData({commits:[{sha:'a',short_sha:'a123456',subject:'Ajustar formulário de cadastro',author:'Desenvolvedor',date:'2026-10-25',location:'local'},{sha:'b',short_sha:'b123456',subject:'Corrigir validação dos dados',author:'Desenvolvedor',date:'2026-10-25',location:'branch'},{sha:'c',short_sha:'c123456',subject:'Atualizar navegação principal',author:'Desenvolvedor',date:'2026-10-25',location:'main'}]});
    });
    await page.screenshot({ path:fileURLToPath(new URL('../preview-deployments.png',import.meta.url)), fullPage:true });
  } finally { await browser.close(); }
});
