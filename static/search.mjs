const LIST_OPERATORS = new Set(["project", "model", "has"]);
const DATE_OPERATORS = new Set(["before", "after"]);

export function tokenize(input) {
  const out = [];
  if (!input) return out;
  let buf = "";
  let inQuote = false;
  for (const ch of input) {
    if (ch === '"') {
      inQuote = !inQuote;
      continue;
    }
    if (!inQuote && /\s/.test(ch)) {
      if (buf) {
        out.push(buf);
        buf = "";
      }
      continue;
    }
    buf += ch;
  }
  if (buf) out.push(buf);
  return out;
}

export function parseQuery(raw) {
  const result = {
    terms: [],
    operators: { project: [], model: [], has: [], before: null, after: null },
  };
  if (typeof raw !== "string" || raw.length === 0) return result;

  for (const tok of tokenize(raw)) {
    const colon = tok.indexOf(":");
    if (colon > 0) {
      const key = tok.slice(0, colon).toLowerCase();
      const value = tok.slice(colon + 1);
      if (value && LIST_OPERATORS.has(key)) {
        result.operators[key].push(value.toLowerCase());
        continue;
      }
      if (value && DATE_OPERATORS.has(key)) {
        const d = new Date(value);
        if (!Number.isNaN(d.getTime())) {
          result.operators[key] = d;
          continue;
        }
      }
    }
    result.terms.push(tok.toLowerCase());
  }
  return result;
}

function lc(value) {
  return typeof value === "string" ? value.toLowerCase() : "";
}

function sessionHasFlag(session, flag) {
  const usage = session.usage || {};
  switch (flag) {
    case "tool":
      return (session.tool_count || 0) > 0;
    case "cache":
      return (usage.cache_read || 0) + (usage.cache_creation || 0) > 0;
    case "model":
      return Boolean(session.model);
    case "title":
      return Boolean(session.title);
    default:
      return true;
  }
}

export function matches(session, parsed) {
  if (!session || !parsed) return true;

  const project = session._project || {};
  const fields = [
    lc(session.title),
    lc(session.snippet),
    lc(session.id),
    lc(session.cwd),
    lc(session.model),
    lc(project.display_path),
    lc(project.sanitized_name),
  ];

  for (const term of parsed.terms) {
    if (!fields.some((f) => f.includes(term))) return false;
  }

  const projectFields = [lc(project.display_path), lc(project.sanitized_name)];
  if (parsed.operators.project.length > 0) {
    const ok = parsed.operators.project.some((v) =>
      projectFields.some((f) => f.includes(v)),
    );
    if (!ok) return false;
  }

  if (parsed.operators.model.length > 0) {
    const model = lc(session.model);
    if (!parsed.operators.model.some((v) => model.includes(v))) return false;
  }

  for (const flag of parsed.operators.has) {
    if (!sessionHasFlag(session, flag)) return false;
  }

  if (parsed.operators.before) {
    if (!session.last_at) return false;
    const ts = new Date(session.last_at);
    if (Number.isNaN(ts.getTime()) || ts >= parsed.operators.before) return false;
  }
  if (parsed.operators.after) {
    if (!session.last_at) return false;
    const ts = new Date(session.last_at);
    if (Number.isNaN(ts.getTime()) || ts <= parsed.operators.after) return false;
  }

  return true;
}

const HTML_ENTITIES = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" };

export function escapeHtml(s) {
  if (s == null) return "";
  return String(s).replace(/[&<>"']/g, (ch) => HTML_ENTITIES[ch]);
}

function escapeRegex(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

export function highlightSegments(text, terms) {
  if (text == null) return [{ text: "", match: false }];
  const source = String(text);
  const valid = (terms || []).filter((t) => typeof t === "string" && t.length > 0);
  if (valid.length === 0 || source.length === 0) {
    return [{ text: source, match: false }];
  }
  const pattern = valid.map(escapeRegex).join("|");
  const re = new RegExp(pattern, "gi");

  const segments = [];
  let last = 0;
  for (const m of source.matchAll(re)) {
    if (m[0].length === 0) continue;
    if (m.index > last) {
      segments.push({ text: source.slice(last, m.index), match: false });
    }
    segments.push({ text: m[0], match: true });
    last = m.index + m[0].length;
  }
  if (last < source.length) {
    segments.push({ text: source.slice(last), match: false });
  }
  if (segments.length === 0) {
    return [{ text: source, match: false }];
  }
  return segments;
}

export function highlight(text, terms) {
  if (text == null || text === "") return "";
  return highlightSegments(String(text), terms)
    .map((seg) =>
      seg.match
        ? "<mark>" + escapeHtml(seg.text) + "</mark>"
        : escapeHtml(seg.text),
    )
    .join("");
}

export function segmentsFromRanges(text, ranges) {
  if (text == null) return [{ text: "", match: false }];
  const source = String(text);
  const chars = Array.from(source);
  if (chars.length === 0 || !ranges || ranges.length === 0) {
    return [{ text: source, match: false }];
  }
  const sorted = [...ranges]
    .map(([s, e]) => [Math.max(0, s), Math.min(chars.length, e)])
    .filter(([s, e]) => s < e && s < chars.length)
    .sort((a, b) => a[0] - b[0]);

  if (sorted.length === 0) return [{ text: source, match: false }];

  const segments = [];
  let last = 0;
  for (const [s, e] of sorted) {
    if (s < last) continue;
    if (s > last) {
      segments.push({ text: chars.slice(last, s).join(""), match: false });
    }
    segments.push({ text: chars.slice(s, e).join(""), match: true });
    last = e;
  }
  if (last < chars.length) {
    segments.push({ text: chars.slice(last).join(""), match: false });
  }
  return segments;
}
