export function boardPage(): string {
  return `<!doctype html>
<html lang="de">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>MiniBoard · dctx0 Tutorial</title>
  <style>
    :root{font-family:system-ui,sans-serif;color:#1e293b;background:#f5f5f0;color-scheme:light}
    *{box-sizing:border-box}body{margin:0}main{max-width:1000px;margin:auto;padding:64px 24px}
    header{display:flex;align-items:center;justify-content:space-between;gap:24px;margin-bottom:44px}
    .eyebrow{font-size:12px;letter-spacing:.12em;text-transform:uppercase;color:#64748b;font-weight:650}
    h1{font-size:44px;letter-spacing:-.045em;margin:8px 0}p{line-height:1.6;color:#64748b}
    button{font:inherit;cursor:pointer;border:1px solid #cbd5c6;border-radius:12px;background:white;color:inherit;padding:12px 18px}
    button:hover{border-color:#527564;background:#f1f7f3}button:focus-visible{outline:3px solid #86bba0;outline-offset:3px}
    button:disabled{cursor:wait;opacity:.6}.layout{display:grid;grid-template-columns:1.3fr 1fr;gap:24px}
    .panel{background:white;border:1px solid #e2e5dd;border-radius:20px;padding:24px;min-height:280px}
    .panel-heading{display:flex;align-items:center;justify-content:space-between;margin-bottom:20px}
    h2{font-size:18px;margin:0}.count{background:#edf5ef;color:#3c674e;border-radius:20px;font-size:13px;padding:5px 10px}
    ul{list-style:none;margin:0;padding:0}li+li{margin-top:10px}.task{width:100%;display:flex;align-items:center;text-align:left;gap:12px;padding:16px}
    .task-row{display:flex;align-items:center;gap:12px}.task{min-width:0}.task-label{min-width:0;overflow-wrap:anywhere}.task-row input{width:20px;height:20px;accent-color:#527564;cursor:pointer;flex-shrink:0}.done .task-label{text-decoration:line-through;color:#84918b}
    .badge{font-size:11px;color:#527564;margin-left:auto;white-space:nowrap}.arrow{margin-left:0!important}
    form{display:flex;gap:10px;margin-bottom:20px}#new-title{font:inherit;min-width:0;width:100%;padding:12px;border:1px solid #cbd5c6;border-radius:12px}#new-title:focus{outline:2px solid #86bba0}form button{white-space:nowrap}
    .dot{width:10px;height:10px;border:2px solid #709780;border-radius:50%;flex-shrink:0}.arrow{margin-left:auto;color:#94a3b8}
    #detail-title{font-size:24px;margin:24px 0 12px;overflow-wrap:anywhere}code{display:block;font-size:12px;line-height:1.7;overflow-wrap:anywhere;color:#64748b}
    #status{min-height:26px;font-size:14px}footer{margin-top:28px;font-size:12px;color:#84918b}
    @media(max-width:650px){main{padding:32px 18px}header{align-items:flex-start;flex-wrap:wrap}h1{font-size:34px}.layout{grid-template-columns:minmax(0,1fr)}.panel{min-height:200px}#refresh{padding:10px}}
  </style>
</head>
<body>
<main>
  <header><div><span class="eyebrow">Das Tutorial · Schritt 16</span><h1>Dein MiniBoard.</h1><p>Ein kleiner Ort für die nächsten Schritte.</p></div><button id="refresh" type="button">Aktualisieren</button></header>
  <div class="layout">
    <section class="panel" aria-labelledby="tasks-heading"><div class="panel-heading"><h2 id="tasks-heading">Aufgaben</h2><span class="count" id="count">…</span></div><form id="new-task"><input id="new-title" aria-label="Neue Aufgabe" placeholder="Was steht als Nächstes an?" maxlength="16" required><button id="create" type="submit">Anlegen</button></form><ul id="tasks"></ul><p id="status" role="status" aria-live="polite">Aufgaben werden geladen …</p></section>
    <section class="panel" aria-labelledby="details-heading"><h2 id="details-heading">Ein Blick ins Detail</h2><p id="hint">Wähle eine Aufgabe aus der Liste.</p><h3 id="detail-title"></h3><code id="detail-id"></code></section>
  </div>
  <footer>MiniBoard · Lokal und übersichtlich.</footer>
</main>
<script>
  const tasks = document.getElementById('tasks'), status = document.getElementById('status');
  const refresh = document.getElementById('refresh'), count = document.getElementById('count');
  const hint = document.getElementById('hint'), title = document.getElementById('detail-title'), id = document.getElementById('detail-id');
  const form = document.getElementById('new-task'), input = document.getElementById('new-title'), create = document.getElementById('create');
  let selection = 0;
  async function writeTask(path, method, task) {
    const response = await fetch(path, {method, headers: {'content-type':'application/json'}, body: JSON.stringify(task)});
    if (!response.ok) throw new Error('Änderung konnte nicht gespeichert werden. Bitte erneut versuchen.');
    return response.json();
  }
  async function selectTask(taskId) {
    const current = ++selection;
    hint.textContent = 'Details werden geladen …'; title.textContent = ''; id.textContent = '';
    try {
      const response = await fetch('/tasks/' + encodeURIComponent(taskId));
      if (!response.ok) throw new Error('Die Aufgabe konnte nicht geladen werden.');
      const task = await response.json();
      if (current !== selection) return;
      hint.textContent = 'Deine ausgewählte Aufgabe'; title.textContent = task.title; id.textContent = task.id;
    } catch (error) { if (current === selection) hint.textContent = error.message; }
  }
  async function loadTasks() {
    refresh.disabled = true; status.textContent = 'Aufgaben werden geladen …';
    ++selection; hint.textContent = 'Wähle eine Aufgabe aus der Liste.'; title.textContent = ''; id.textContent = '';
    try {
      const response = await fetch('/board/tasks');
      if (!response.ok) throw new Error('Die Aufgaben konnten nicht geladen werden. Bitte erneut versuchen.');
      const list = await response.json();
      tasks.replaceChildren(); count.textContent = String(list.length);
      for (const task of list) {
        const item = document.createElement('li'), button = document.createElement('button');
        item.className = 'task-row' + (task.status === 'Done' ? ' done' : '');
        const completed = document.createElement('input'); completed.type = 'checkbox'; completed.checked = task.status !== 'Open';
        completed.disabled = task.status === 'Archived'; completed.setAttribute('aria-label', task.title + ' erledigt');
        completed.addEventListener('change', async () => {
          completed.disabled = true;
          try { await writeTask('/tasks/' + encodeURIComponent(task.id), 'PATCH', {...task, status: completed.checked ? 'Done' : 'Open'}); await loadTasks(); }
          catch (error) { completed.checked = task.status !== 'Open'; status.textContent = error.message; }
          finally { completed.disabled = task.status === 'Archived'; }
        });
        button.type = 'button'; button.className = 'task';
        const dot = document.createElement('span'), label = document.createElement('span'), arrow = document.createElement('span'), badge = document.createElement('span');
        dot.className = 'dot'; dot.setAttribute('aria-hidden', 'true'); label.textContent = task.title;
        label.className = 'task-label'; badge.className = 'badge'; badge.textContent = task.status === 'Open' ? 'Offen' : task.status === 'Done' ? 'Erledigt' : 'Archiviert';
        arrow.className = 'arrow'; arrow.textContent = '↗'; arrow.setAttribute('aria-hidden', 'true');
        button.append(dot, label, badge, arrow); button.addEventListener('click', () => selectTask(task.id));
        item.append(completed, button); tasks.append(item);
      }
      status.textContent = list.length ? 'Alles auf dem aktuellen Stand.' : 'Noch keine Aufgaben vorhanden.';
    } catch (error) { tasks.replaceChildren(); count.textContent = '–'; status.textContent = error.message; }
    finally { refresh.disabled = false; }
  }
  refresh.addEventListener('click', loadTasks);
  form.addEventListener('submit', async event => {
    event.preventDefault();
    if (!input.value.trim()) { status.textContent = 'Bitte einen Titel eingeben.'; return; }
    create.disabled = true;
    try { await writeTask('/tasks', 'POST', {id: crypto.randomUUID(), title: input.value, status: 'Open'}); input.value = ''; await loadTasks(); }
    catch (error) { status.textContent = error.message; }
    finally { create.disabled = false; }
  });
  loadTasks();
</script>
</body>
</html>`;
}
