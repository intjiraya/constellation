const state = {
  projects: [],
  activeProject: null,
  sessions: [],
  activeSessionId: null,
  fullSession: null,
  searchQuery: "",
  termInst: null,
  termWs: null,
  termFitAddon: null,
  termResizeHandler: null,
};

async function api(path, opts = {}) {
  const res = await fetch(path, opts);
  if (!res.ok) {
    throw new Error(`${res.status} ${res.statusText} — ${path}`);
  }
  return res.json();
}

function wsUrl(path) {
  const proto = location.protocol === "https:" ? "wss:" : "ws:";
  return `${proto}//${location.host}${path}`;
}

const API = {
  stats:    ()      => api("/api/stats"),
  reindex:  ()      => api("/api/reindex", { method: "POST" }),
  projects: ()      => api("/api/projects"),
  sessions: (proj)  => api(`/api/projects/${encodeURIComponent(proj)}/sessions`),
  session:  (id)    => api(`/api/sessions/${encodeURIComponent(id)}`),
  resumeWsUrl:  (id, fork) =>
    wsUrl(`/api/sessions/${encodeURIComponent(id)}/pty${fork ? "?fork=true" : ""}`),
  newChatWsUrl: (proj) => wsUrl(`/api/projects/${encodeURIComponent(proj)}/new-chat`),
};

const $  = (sel) => document.querySelector(sel);
const $$ = (sel) => document.querySelectorAll(sel);

function el(tag, attrs = {}, ...children) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "class") node.className = v;
    else if (k.startsWith("on")) node.addEventListener(k.slice(2), v);
    else if (v !== null && v !== undefined) node.setAttribute(k, v);
  }
  for (const c of children) {
    if (c == null) continue;
    if (typeof c === "string") node.appendChild(document.createTextNode(c));
    else node.appendChild(c);
  }
  return node;
}

function setSafeHtml(node, html) {
  if (!window.DOMPurify) {
    node.textContent = html;
    return;
  }
  node.innerHTML = DOMPurify.sanitize(html, { USE_PROFILES: { html: true } });
}

function relTime(iso) {
  if (!iso) return "";
  const d = new Date(iso);
  const diff = (Date.now() - d.getTime()) / 1000;
  if (diff < 60)         return "just now";
  if (diff < 3600)       return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400)      return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 86400 * 7)  return `${Math.floor(diff / 86400)}d ago`;
  return d.toLocaleDateString(undefined, { day: "2-digit", month: "short" });
}

function absTime(iso) {
  if (!iso) return "";
  const d = new Date(iso);
  return d.toLocaleString(undefined, {
    year: "numeric", month: "2-digit", day: "2-digit",
    hour: "2-digit", minute: "2-digit",
  });
}

function timeOnly(iso) {
  if (!iso) return "";
  const d = new Date(iso);
  return d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

function fmtNum(n) {
  if (n == null) return "0";
  if (n >= 1000) return (n / 1000).toFixed(n >= 10000 ? 0 : 1) + "k";
  return String(n);
}

function fmtTok(n) {
  if (n == null) return "0";
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(n >= 10_000_000 ? 0 : 1) + "M";
  if (n >= 1_000)     return (n / 1_000).toFixed(n >= 10_000 ? 0 : 1) + "k";
  return String(n);
}

function tokensTotal(usage) {
  if (!usage) return 0;
  return (usage.input || 0) + (usage.cache_creation || 0)
       + (usage.cache_read || 0) + (usage.output || 0);
}
function tokensInput(usage) {
  if (!usage) return 0;
  return (usage.input || 0) + (usage.cache_creation || 0) + (usage.cache_read || 0);
}
function cacheHitPct(usage) {
  if (!usage) return null;
  const totalIn = tokensInput(usage);
  if (totalIn === 0) return null;
  return Math.round((usage.cache_read || 0) / totalIn * 100);
}

function dayLabel(iso) {
  if (!iso) return "older";
  const d = new Date(iso);
  const today = new Date(); today.setHours(0, 0, 0, 0);
  const day = new Date(d); day.setHours(0, 0, 0, 0);
  const diff = (today.getTime() - day.getTime()) / 86400000;
  if (diff === 0)  return "today";
  if (diff === 1)  return "yesterday";
  if (diff < 7)    return "earlier this week";
  if (diff < 31)   return "earlier this month";
  return d.toLocaleDateString(undefined, { year: "numeric", month: "short" });
}

if (window.marked) {
  marked.setOptions({
    gfm: true,
    breaks: false,
    headerIds: false,
    mangle: false,
    highlight: (code, lang) => {
      if (!window.hljs) return code;
      try {
        if (lang && hljs.getLanguage(lang)) {
          return hljs.highlight(code, { language: lang, ignoreIllegals: true }).value;
        }
        return hljs.highlightAuto(code).value;
      } catch {
        return code;
      }
    },
  });
}

function mdToSafeHtml(text) {
  if (!text) return "";

if (!window.DOMPurify) {
    return escapeHtmlText(text);
  }
  const raw = window.marked ? marked.parse(text) : text;
  return DOMPurify.sanitize(raw, { USE_PROFILES: { html: true } });
}

function escapeHtmlText(s) {
  const div = document.createElement("div");
  div.textContent = s;
  return div.innerHTML;
}

function renderRail() {
  const list = $("#proj-list");
  list.innerHTML = "";
  for (const p of state.projects) {
    const nameSpan = document.createElement("span");
    nameSpan.className = "name";
    appendPath(nameSpan, p.display_path);

    const item = el(
      "div",
      {
        class: "r-item" + (state.activeProject === p.sanitized_name ? " active" : ""),
        "data-project": p.sanitized_name,
        title: p.cwd,
        onclick: () => selectProject(p.sanitized_name),
      },
      nameSpan,
      el("span", { class: "n" }, String(p.session_count)),
    );
    list.appendChild(item);
  }
  for (const r of $$('.r-item[data-project="*"]')) {
    r.classList.toggle("active", state.activeProject === "*");
    r.onclick = () => selectProject("*");
  }
}

function appendPath(parent, path) {
  if (path == null) {
    parent.textContent = "—";
    return;
  }
  if (path.startsWith("~/")) {
    const muted = document.createElement("span");
    muted.className = "slash";
    muted.textContent = "~/";
    parent.appendChild(muted);
    parent.appendChild(document.createTextNode(path.slice(2)));
  } else if (path === "~") {
    const muted = document.createElement("span");
    muted.className = "slash";
    muted.textContent = "~";
    parent.appendChild(muted);
  } else {
    parent.textContent = path;
  }
}

function renderChatList(sessions) {
  const root = $("#chat-list");
  root.innerHTML = "";
  if (!sessions || sessions.length === 0) {
    root.appendChild(el("div", { class: "placeholder" }, "no conversations"));
    return;
  }

  let prevLabel = null;
  for (const s of sessions) {
    const label = dayLabel(s.last_at);
    if (label !== prevLabel) {
      root.appendChild(el("div", { class: "day" }, label));
      prevLabel = label;
    }
    root.appendChild(renderChatRow(s));
  }
}

function renderChatRow(s) {
  const total = tokensTotal(s.usage);
  const metaChildren = [
    el("span", {}, `${s.message_count} msgs`),
    el("span", { class: "sep" }, "·"),
    el("span", { title: absTime(s.last_at) }, relTime(s.last_at)),
  ];
  if (total > 0) {
    const u = s.usage || {};
    metaChildren.push(el("span", { class: "sep" }, "·"));
    metaChildren.push(el(
      "span",
      { title: `${(u.input ?? 0).toLocaleString()} fresh · ${(u.cache_creation ?? 0).toLocaleString()} cache-create · ${(u.cache_read ?? 0).toLocaleString()} cache-read · ${(u.output ?? 0).toLocaleString()} output` },
      `${fmtTok(total)} tok`,
    ));
  }
  if (s.model) {
    metaChildren.push(el("span", { class: "sep" }, "·"));
    metaChildren.push(el("span", {}, s.model));
  }
  return el(
    "article",
    {
      class: "chat" + (state.activeSessionId === s.id ? " active" : ""),
      "data-id": s.id,
      onclick: () => selectSession(s.id),
    },
    el("div", { class: "title", title: s.title }, s.title || "(untitled)"),
    el("div", { class: "snippet" }, s.snippet || "—"),
    el("div", { class: "meta" }, ...metaChildren),
  );
}

function renderListHead(label, sessions) {
  const pathNode = $("#list-path");
  pathNode.innerHTML = "";
  const leaf = document.createElement("span");
  leaf.className = "leaf";
  leaf.textContent = label;
  pathNode.appendChild(leaf);

  const msgs = sessions.reduce((acc, s) => acc + s.message_count, 0);
  const tools = sessions.reduce((acc, s) => acc + s.tool_count, 0);
  $("#list-stats").textContent =
    `${sessions.length} chat${sessions.length === 1 ? "" : "s"} · ${msgs} msgs · ${tools} tool calls`;
}

function renderPreview(s) {
  if (!s) return;
  $("#stage-empty").style.display = "none";
  $("#stage-loaded").style.display = "flex";

  const pathNode = $("#stage-path");
  pathNode.innerHTML = "";
  pathNode.appendChild(textSpan("seg", s.cwd || "—"));
  pathNode.appendChild(textSpan("sep", "·"));
  pathNode.appendChild(textSpan("seg id", s.id.slice(0, 18)));

  $("#stage-title").textContent = s.title || "(untitled)";
  $("#stage-title").title = s.title || "";

  const meta = $("#stage-meta");
  meta.innerHTML = "";
  meta.appendChild(metaPart(fmtNum(s.message_count), "messages"));
  meta.appendChild(metaPart(fmtNum(s.tool_count), "tool calls"));
  const totalIn = tokensInput(s.usage);
  if (totalIn > 0) {
    meta.appendChild(metaPart(fmtTok(totalIn), "in"));
    meta.appendChild(metaPart(fmtTok((s.usage && s.usage.output) || 0), "out"));
    const pct = cacheHitPct(s.usage);
    if (pct != null) meta.appendChild(metaPart(`${pct}%`, "cached"));
  }
  if (s.model)     meta.appendChild(metaPart(s.model, ""));
  if (s.last_at)   meta.appendChild(metaPart(absTime(s.last_at), ""));

  const preview = $("#preview");
  preview.innerHTML = "";
  preview.style.display = "block";

  if (!s.turns || s.turns.length === 0) {
    preview.appendChild(el("div", { class: "placeholder" }, "(empty session)"));
    return;
  }
  for (const t of s.turns) {
    preview.appendChild(renderTurn(t));
  }
  
  requestAnimationFrame(() => {
    preview.scrollTop = preview.scrollHeight;
  });
}

function textSpan(cls, text) {
  return el("span", { class: cls }, text);
}

function metaPart(bold, label) {
  const span = document.createElement("span");
  const b = document.createElement("b");
  b.textContent = bold;
  span.appendChild(b);
  if (label) span.appendChild(document.createTextNode(" " + label));
  return span;
}

function renderTurn(t) {
  const klass = t.role === "user" ? "user" : t.role === "assistant" ? "asst" : "tool";
  const head = el(
    "div",
    { class: "msg-head" },
    el("span", { class: "role" }, t.role),
    t.model ? el("span", { class: "model" }, t.model) : null,
    el("span", { class: "ts" }, timeOnly(t.timestamp)),
  );
  const body = el("div", { class: "msg-body" });
  for (const b of t.blocks) {
    const node = renderBlock(b);
    if (node) body.appendChild(node);
  }
  if (body.childElementCount === 0) {
    body.appendChild(renderEmptyTurnPlaceholder(t));
  }
  return el("div", { class: `msg ${klass}` }, head, body);
}

function renderEmptyTurnPlaceholder(t) {
  const hadThinking = (t.blocks || []).some(b => b.kind === "thinking");
  if (t.role === "assistant") {
    const wrap = el("div", { class: "empty-turn" });
    wrap.appendChild(el("span", { class: "empty-icon" }, "✦"));
    const text = document.createElement("div");
    text.appendChild(el("div", { class: "empty-title" }, "model paused to think"));
    text.appendChild(el(
      "div",
      { class: "empty-sub" },
      hadThinking
        ? "extended reasoning happened here; the trace was stored encrypted"
        : "claude produced no visible output in this turn",
    ));
    wrap.appendChild(text);
    return wrap;
  }
  return el("div", { class: "placeholder" }, "(no content)");
}

function renderBlock(b) {
  if (b.kind === "text") {
    const wrap = document.createElement("div");
    setSafeHtml(wrap, mdToSafeHtml(b.text || ""));
    return wrap;
  }
  if (b.kind === "thinking") {
    const text = (b.text || "").trim();
    if (!text) return null;  
    return el("details", { class: "thinking" },
      el("summary", {}, `thinking (${text.length} chars)`),
      el("div", { class: "content" }, text),
    );
  }
  if (b.kind === "tool_use") {
    const args = JSON.stringify(b.tool_input || {}, null, 2);
    return el("div", { class: "tool-block" },
      el("div", { class: "name" }, `▸ ${b.tool_name || "tool"}`),
      el("pre", { class: "input" }, args),
    );
  }
  if (b.kind === "tool_result") {
    return el("div", { class: "tool-block" },
      el("div", { class: "name" }, b.is_error ? "◂ result (error)" : "◂ result"),
      el("pre", { class: `result${b.is_error ? " error" : ""}` }, b.tool_output || ""),
    );
  }
  return el("div", {}, JSON.stringify(b));
}

async function selectProject(sanitized) {
  state.activeProject = sanitized;
  state.activeSessionId = null;
  renderRail();

  let sessions = [];
  let label = "—";
  if (sanitized === "*") {
    sessions = await collectRecent();
    label = "recent across all projects";
  } else {
    sessions = await API.sessions(sanitized);
    const project = state.projects.find(p => p.sanitized_name === sanitized);
    label = project ? project.display_path : sanitized;
  }

  state.sessions = sessions;
  applySearchFilter();
  renderListHead(label, state.sessions);
}

async function collectRecent(limit = 50) {
  const topProjects = state.projects.slice(0, 10);

const results = await Promise.all(
    topProjects.map(p => API.sessions(p.sanitized_name).catch(() => [])),
  );
  const all = results.flat();
  all.sort((a, b) => new Date(b.last_at || 0) - new Date(a.last_at || 0));
  return all.slice(0, limit);
}

async function selectSession(id) {
  state.activeSessionId = id;
  $$("#chat-list .chat").forEach(r => {
    r.classList.toggle("active", r.dataset.id === id);
  });
  closeTerminal();
  try {
    const full = await API.session(id);
    state.fullSession = full;
    renderPreview(full);
  } catch (e) {
    console.error(e);
    $("#stage-empty").style.display = "none";
    $("#stage-loaded").style.display = "flex";
    const p = $("#preview");
    p.innerHTML = "";
    p.appendChild(el("div", { class: "placeholder" }, "failed to load: " + e.message));
  }
}

function applySearchFilter() {
  const q = state.searchQuery.trim().toLowerCase();
  let filtered = state.sessions;
  if (q) {
    filtered = state.sessions.filter(s => {
      return (s.title || "").toLowerCase().includes(q)
        || (s.snippet || "").toLowerCase().includes(q)
        || (s.id || "").toLowerCase().includes(q);
    });
  }
  renderChatList(filtered);
}

function openTerminal(sessionId, { fork = false } = {}) {
  if (!sessionId) return;
  _attachTerminal(API.resumeWsUrl(sessionId, fork), { onClose: null });
}

function openNewChat(project) {
  if (!project) {
    toast("select a project first (left sidebar)");
    return;
  }
  
  $("#stage-empty").style.display = "none";
  $("#stage-loaded").style.display = "flex";

  const pathNode = $("#stage-path");
  pathNode.innerHTML = "";
  pathNode.appendChild(el("span", { class: "seg" }, project.cwd || project.sanitized_name));
  pathNode.appendChild(el("span", { class: "sep" }, "·"));
  pathNode.appendChild(el("span", { class: "seg id" }, "new chat"));

  $("#stage-title").textContent = `new chat in ${project.display_path}`;
  $("#stage-title").title = project.cwd || "";

  const meta = $("#stage-meta");
  meta.innerHTML = "";
  meta.appendChild(el("span", {}, `starting claude in ${project.cwd}…`));

$("#preview").innerHTML = "";
  $("#btn-fork").style.display = "none";
  $("#btn-resume").style.display = "none";

  _attachTerminal(API.newChatWsUrl(project.sanitized_name), {
    onClose: async () => {
      
      try {
        await API.reindex();
        await refreshStats();
        await loadProjects(project.sanitized_name);
      } catch {  }
    },
  });
}

const TERM_OPTIONS = {
  fontFamily: '"Geist Mono", ui-monospace, Menlo, monospace',
  fontSize: 14,
  lineHeight: 1.25,
  cursorBlink: true,
  scrollback: 10000,
  theme: {
    background: "#22272e",
    foreground: "#cdd9e5",
    cursor:     "#539bf5",
    black:      "#22272e",
    red:        "#f47067",
    green:      "#57ab5a",
    yellow:     "#c69026",
    blue:       "#539bf5",
    magenta:    "#dcbdfb",
    cyan:       "#39c5cf",
    white:      "#cdd9e5",
    brightBlack:   "#545d68",
    brightRed:     "#ff938a",
    brightGreen:   "#8ddb8c",
    brightYellow:  "#e0b65b",
    brightBlue:    "#6cb6ff",
    brightMagenta: "#dcbdfb",
    brightCyan:    "#39c5cf",
    brightWhite:   "#ffffff",
  },
};

function _attachTerminal(socketUrl, { onClose }) {
  closeTerminal({ resetButtons: false });

  $("#preview").style.display = "none";
  $("#terminal-wrap").style.display = "flex";
  $("#btn-close-term").style.display = "inline-flex";
  const resumeBtn = $("#btn-resume");
  resumeBtn.disabled = true;
  resumeBtn.textContent = "● live";

  const term = new Terminal(TERM_OPTIONS);
  const fit = new FitAddon.FitAddon();
  term.loadAddon(fit);
  if (window.WebLinksAddon) term.loadAddon(new WebLinksAddon.WebLinksAddon());
  term.open($("#terminal"));
  fit.fit();

  state.termInst = term;
  state.termFitAddon = fit;

  const ws = new WebSocket(socketUrl);
  ws.binaryType = "arraybuffer";
  state.termWs = ws;
  state.termOnClose = onClose || null;

  ws.onopen = () => {
    const { cols, rows } = term;
    ws.send(JSON.stringify({ type: "resize", cols, rows }));
  };
  ws.onmessage = (ev) => {
    if (ev.data instanceof ArrayBuffer) {
      term.write(new Uint8Array(ev.data));
    } else if (typeof ev.data === "string") {
      term.write(ev.data);
    }
  };
  ws.onclose = () => {
    term.write("\r\n\x1b[90m[session disconnected]\x1b[0m\r\n");
    resumeBtn.disabled = false;
    resumeBtn.textContent = "▶ resume";
    if (typeof state.termOnClose === "function") {
      const cb = state.termOnClose;
      state.termOnClose = null;
      try { cb(); } catch (e) { console.error(e); }
    }
  };
  ws.onerror = () => {
    term.write("\r\n\x1b[31m[connection error]\x1b[0m\r\n");
  };

  term.onData(data => {
    if (ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: "input", data }));
    }
  });

  term.onResize(({ cols, rows }) => {
    if (ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: "resize", cols, rows }));
    }
  });

  const onResize = () => { try { fit.fit(); } catch {} };
  window.addEventListener("resize", onResize);
  state.termResizeHandler = onResize;

  term.focus();
}

function closeTerminal({ resetButtons = true } = {}) {
  if (state.termWs) {
    try { state.termWs.close(); } catch {}
    state.termWs = null;
  }
  if (state.termInst) {
    try { state.termInst.dispose(); } catch {}
    state.termInst = null;
  }
  if (state.termResizeHandler) {
    window.removeEventListener("resize", state.termResizeHandler);
    state.termResizeHandler = null;
  }
  $("#terminal-wrap").style.display = "none";
  $("#terminal").innerHTML = "";
  $("#preview").style.display = "block";
  if (resetButtons) {
    $("#btn-close-term").style.display = "none";
    $("#btn-fork").style.display = "";
    $("#btn-resume").style.display = "";
    const resumeBtn = $("#btn-resume");
    resumeBtn.disabled = false;
    resumeBtn.textContent = "▶ resume";
  }
}

function toast(msg, kind = "info") {
  let host = $("#toast-host");
  if (!host) {
    host = el("div", { id: "toast-host" });
    document.body.appendChild(host);
  }
  const t = el("div", { class: "toast toast-" + kind }, msg);
  host.appendChild(t);
  setTimeout(() => { t.classList.add("show"); }, 10);
  setTimeout(() => {
    t.classList.remove("show");
    setTimeout(() => t.remove(), 250);
  }, 2400);
}

async function refreshStats() {
  try {
    const s = await API.stats();
    const sysEl = $("#sys");
    sysEl.classList.toggle("scanning", s.scanning);
    sysEl.classList.remove("error");
    const totalTok = tokensTotal(s.total_usage);
    const main = s.scanning ? "scanning…" : `${s.sessions} indexed`;
    const tokPart = totalTok > 0 ? ` · ${fmtTok(totalTok)} tok` : "";
    $("#sys-text").textContent = main + tokPart;
    const u = s.total_usage || {};
    const tokDetail = totalTok > 0
      ? `\ninput ${(u.input ?? 0).toLocaleString()} · cache-create ${(u.cache_creation ?? 0).toLocaleString()} · cache-read ${(u.cache_read ?? 0).toLocaleString()} · output ${(u.output ?? 0).toLocaleString()}`
      : "";
    sysEl.title = (s.last_scan ? `last scan: ${absTime(s.last_scan)}` : "") + tokDetail;
    $("#all-count").textContent = String(s.sessions);
    $("#proj-count").textContent = String(s.projects);
  } catch (e) {
    const sysEl = $("#sys");
    sysEl.classList.remove("scanning");
    sysEl.classList.add("error");
    $("#sys-text").textContent = "offline";
  }
}

async function loadProjects(preferProject = null) {
  state.projects = await API.projects();
  renderRail();
  $("#all-count").textContent = String(
    state.projects.reduce((a, p) => a + p.session_count, 0),
  );
  $("#proj-count").textContent = String(state.projects.length);
  if (state.projects.length === 0) return;
  const target =
    (preferProject && state.projects.find(p => p.sanitized_name === preferProject))
    || state.projects[0];
  await selectProject(target.sanitized_name);
}

function bindUi() {
  $("#search").addEventListener("input", (e) => {
    state.searchQuery = e.target.value;
    applySearchFilter();
  });

  document.addEventListener("keydown", (e) => {
    const mod = e.metaKey || e.ctrlKey;
    if (mod && e.key.toLowerCase() === "k") {
      e.preventDefault();
      $("#search").focus();
      $("#search").select();
    }
    if (e.key === "Escape" && document.activeElement === $("#search")) {
      $("#search").blur();
    }
    if (e.key === "Enter"
        && state.fullSession
        && !state.termWs
        && document.activeElement !== $("#search")) {
      openTerminal(state.fullSession.id);
    }
  });

  $("#btn-resume").addEventListener("click", () => {
    if (state.fullSession) openTerminal(state.fullSession.id);
  });
  $("#btn-fork").addEventListener("click", () => {
    if (state.fullSession) openTerminal(state.fullSession.id, { fork: true });
  });
  $("#btn-close-term").addEventListener("click", () => closeTerminal());
  $("#btn-reindex").addEventListener("click", async () => {
    const btn = $("#btn-reindex");
    btn.disabled = true;
    btn.textContent = "↻ scanning…";
    try {
      await API.reindex();
      await refreshStats();
      await loadProjects(state.activeProject);
    } finally {
      btn.disabled = false;
      btn.textContent = "↻ reindex";
    }
  });
  $("#btn-new-chat").addEventListener("click", () => {
    const proj = state.projects.find(p => p.sanitized_name === state.activeProject);
    openNewChat(proj || null);
  });
}

async function main() {
  bindUi();
  await refreshStats();
  await loadProjects();
  setInterval(refreshStats, 15000);
}

main().catch(err => {
  console.error(err);
  $("#sys-text").textContent = "init failed: " + err.message;
  $("#sys").classList.add("error");
});
