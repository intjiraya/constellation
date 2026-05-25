import { test } from "node:test";
import assert from "node:assert/strict";

import {
  tokenize,
  parseQuery,
  matches,
  highlight,
  highlightSegments,
  segmentsFromRanges,
} from "../static/search.mjs";

test("tokenize splits by whitespace", () => {
  assert.deepEqual(tokenize("foo bar baz"), ["foo", "bar", "baz"]);
});

test("tokenize keeps quoted strings as one token", () => {
  assert.deepEqual(
    tokenize('foo "merge conflict" bar'),
    ["foo", "merge conflict", "bar"],
  );
});

test("tokenize collapses runs of whitespace", () => {
  assert.deepEqual(tokenize("foo   bar"), ["foo", "bar"]);
});

test("tokenize trims leading and trailing whitespace", () => {
  assert.deepEqual(tokenize("  foo  "), ["foo"]);
});

test("tokenize returns empty array for empty / blank input", () => {
  assert.deepEqual(tokenize(""), []);
  assert.deepEqual(tokenize("   "), []);
});

test("tokenize treats unterminated quote as a single trailing token", () => {
  assert.deepEqual(tokenize('foo "bar baz'), ["foo", "bar baz"]);
});

test("tokenize keeps colon-form operators intact", () => {
  assert.deepEqual(tokenize("project:auth bug"), ["project:auth", "bug"]);
});

test("tokenize allows quotes inside an operator value", () => {
  assert.deepEqual(
    tokenize('project:"my project" foo'),
    ["project:my project", "foo"],
  );
});

test("parseQuery: empty input returns empty defaults", () => {
  const r = parseQuery("");
  assert.deepEqual(r.terms, []);
  assert.deepEqual(r.operators.project, []);
  assert.deepEqual(r.operators.model, []);
  assert.deepEqual(r.operators.has, []);
  assert.equal(r.operators.before, null);
  assert.equal(r.operators.after, null);
});

test("parseQuery: plain term goes to terms, lowercased", () => {
  assert.deepEqual(parseQuery("Auth").terms, ["auth"]);
});

test("parseQuery: multiple plain terms preserved (AND set)", () => {
  assert.deepEqual(parseQuery("auth bug").terms, ["auth", "bug"]);
});

test("parseQuery: project: operator captured", () => {
  const r = parseQuery("project:Web");
  assert.deepEqual(r.terms, []);
  assert.deepEqual(r.operators.project, ["web"]);
});

test("parseQuery: model: and has: operators captured", () => {
  const r = parseQuery("model:OPUS has:tool");
  assert.deepEqual(r.operators.model, ["opus"]);
  assert.deepEqual(r.operators.has, ["tool"]);
});

test("parseQuery: before: parses ISO date", () => {
  const r = parseQuery("before:2026-01-15");
  assert.ok(r.operators.before instanceof Date);
  assert.equal(r.operators.before.toISOString().slice(0, 10), "2026-01-15");
});

test("parseQuery: after: parses ISO date", () => {
  const r = parseQuery("after:2026-01-15");
  assert.ok(r.operators.after instanceof Date);
});

test("parseQuery: mixed terms and operators", () => {
  const r = parseQuery("auth project:web has:tool");
  assert.deepEqual(r.terms, ["auth"]);
  assert.deepEqual(r.operators.project, ["web"]);
  assert.deepEqual(r.operators.has, ["tool"]);
});

test("parseQuery: invalid date falls through to a term", () => {
  const r = parseQuery("before:not-a-date");
  assert.deepEqual(r.terms, ["before:not-a-date"]);
  assert.equal(r.operators.before, null);
});

test("parseQuery: unknown operator stays as a plain term", () => {
  assert.deepEqual(parseQuery("foo:bar").terms, ["foo:bar"]);
});

test("parseQuery: repeated same operator accumulates", () => {
  assert.deepEqual(
    parseQuery("project:web project:api").operators.project,
    ["web", "api"],
  );
});

test("parseQuery: quoted operator value preserves spaces", () => {
  assert.deepEqual(
    parseQuery('project:"my project"').operators.project,
    ["my project"],
  );
});

test("parseQuery: leading-colon token treated as term", () => {
  assert.deepEqual(parseQuery(":foo").terms, [":foo"]);
});

test("parseQuery: operator with empty value falls through to term", () => {
  assert.deepEqual(parseQuery("project:").terms, ["project:"]);
});

test("matches: empty query matches anything", () => {
  assert.equal(matches({ title: "x" }, parseQuery("")), true);
  assert.equal(matches({}, parseQuery("")), true);
});

test("matches: term in title (case-insensitive)", () => {
  assert.equal(matches({ title: "Auth Bug Fix" }, parseQuery("auth")), true);
  assert.equal(matches({ title: "" }, parseQuery("auth")), false);
});

test("matches: term in snippet, id, cwd, model", () => {
  assert.equal(matches({ snippet: "fixing AUTH flow" }, parseQuery("auth")), true);
  assert.equal(matches({ id: "abc-123-def" }, parseQuery("123")), true);
  assert.equal(matches({ cwd: "/home/x/web-app" }, parseQuery("web")), true);
  assert.equal(matches({ model: "claude-opus-4-7" }, parseQuery("opus")), true);
});

test("matches: term in project display_path or sanitized_name", () => {
  const s = { _project: { display_path: "~/code/web-app", sanitized_name: "-home-x-web-app" } };
  assert.equal(matches(s, parseQuery("web")), true);
  assert.equal(matches({ _project: { sanitized_name: "my-thing" } }, parseQuery("thing")), true);
});

test("matches: multi-term AND across fields", () => {
  const s = { title: "Auth fix", snippet: "patches a critical bug" };
  assert.equal(matches(s, parseQuery("auth bug")), true);
  assert.equal(matches(s, parseQuery("auth zzz")), false);
});

test("matches: project: operator", () => {
  const s = { _project: { display_path: "~/web", sanitized_name: "x-web" } };
  assert.equal(matches(s, parseQuery("project:web")), true);
  assert.equal(matches(s, parseQuery("project:api")), false);
});

test("matches: project: OR within same operator", () => {
  const s = { _project: { display_path: "~/web" } };
  assert.equal(matches(s, parseQuery("project:api project:web")), true);
});

test("matches: model: operator", () => {
  const s = { model: "claude-opus-4-7" };
  assert.equal(matches(s, parseQuery("model:opus")), true);
  assert.equal(matches(s, parseQuery("model:sonnet")), false);
});

test("matches: has:tool", () => {
  assert.equal(matches({ tool_count: 5 }, parseQuery("has:tool")), true);
  assert.equal(matches({ tool_count: 0 }, parseQuery("has:tool")), false);
  assert.equal(matches({}, parseQuery("has:tool")), false);
});

test("matches: has:cache", () => {
  assert.equal(matches({ usage: { cache_read: 1000 } }, parseQuery("has:cache")), true);
  assert.equal(matches({ usage: { cache_creation: 1 } }, parseQuery("has:cache")), true);
  assert.equal(matches({ usage: {} }, parseQuery("has:cache")), false);
  assert.equal(matches({}, parseQuery("has:cache")), false);
});

test("matches: has:model", () => {
  assert.equal(matches({ model: "x" }, parseQuery("has:model")), true);
  assert.equal(matches({ model: "" }, parseQuery("has:model")), false);
});

test("matches: has: AND across different flags", () => {
  const s1 = { tool_count: 5, usage: { cache_read: 1 } };
  assert.equal(matches(s1, parseQuery("has:tool has:cache")), true);
  const s2 = { tool_count: 0, usage: { cache_read: 1 } };
  assert.equal(matches(s2, parseQuery("has:tool has:cache")), false);
});

test("matches: unknown has-flag is permissive (does not filter)", () => {
  assert.equal(matches({ title: "x" }, parseQuery("has:flying-cars")), true);
});

test("matches: before: filters by last_at", () => {
  const s = { last_at: "2026-01-10T00:00:00Z" };
  assert.equal(matches(s, parseQuery("before:2026-01-15")), true);
  assert.equal(matches(s, parseQuery("before:2026-01-01")), false);
});

test("matches: after: filters by last_at", () => {
  const s = { last_at: "2026-02-01T00:00:00Z" };
  assert.equal(matches(s, parseQuery("after:2026-01-15")), true);
  assert.equal(matches(s, parseQuery("after:2026-03-01")), false);
});

test("matches: session without last_at fails any date filter", () => {
  assert.equal(matches({}, parseQuery("before:2026-01-15")), false);
  assert.equal(matches({}, parseQuery("after:2026-01-15")), false);
});

test("matches: AND across different operators", () => {
  const s = {
    title: "auth fix",
    model: "opus",
    _project: { display_path: "~/web" },
  };
  assert.equal(matches(s, parseQuery("auth model:opus project:web")), true);
  assert.equal(matches(s, parseQuery("auth model:opus project:api")), false);
});

test("matches: graceful on missing fields", () => {
  assert.equal(matches({}, parseQuery("foo")), false);
});

test("highlight: empty text returns empty string", () => {
  assert.equal(highlight("", ["foo"]), "");
  assert.equal(highlight(null, ["foo"]), "");
  assert.equal(highlight(undefined, ["foo"]), "");
});

test("highlight: empty terms returns escaped text", () => {
  assert.equal(highlight("hello & <world>", []), "hello &amp; &lt;world&gt;");
  assert.equal(highlight("plain", null), "plain");
});

test("highlight: wraps single term", () => {
  assert.equal(highlight("auth fix", ["auth"]), "<mark>auth</mark> fix");
});

test("highlight: case-insensitive but preserves original case", () => {
  assert.equal(highlight("Auth Bug", ["auth"]), "<mark>Auth</mark> Bug");
});

test("highlight: wraps multiple terms", () => {
  assert.equal(
    highlight("auth and bug", ["auth", "bug"]),
    "<mark>auth</mark> and <mark>bug</mark>",
  );
});

test("highlight: escapes HTML in non-matched parts", () => {
  assert.equal(
    highlight("foo <script>", ["foo"]),
    "<mark>foo</mark> &lt;script&gt;",
  );
});

test("highlight: escapes HTML in the matched span", () => {
  assert.equal(highlight("a&b", ["&"]), "a<mark>&amp;</mark>b");
});

test("highlight: treats regex meta-chars as literal", () => {
  assert.equal(
    highlight("foo.bar fooXbar", ["."]),
    "foo<mark>.</mark>bar fooXbar",
  );
});

test("highlight: no match returns escaped text untouched", () => {
  assert.equal(highlight("abc", ["xyz"]), "abc");
  assert.equal(highlight("<b>", ["xyz"]), "&lt;b&gt;");
});

test("highlight: supports Unicode terms", () => {
  assert.equal(
    highlight("Привет мир", ["мир"]),
    "Привет <mark>мир</mark>",
  );
});

test("highlight: ignores empty strings in terms list", () => {
  assert.equal(highlight("auth fix", ["", "auth", ""]), "<mark>auth</mark> fix");
});

test("highlight: handles overlapping terms without infinite loop", () => {
  const out = highlight("foofoo", ["foo", "foofoo"]);
  assert.ok(out.includes("<mark>"));
  assert.ok(out.endsWith("</mark>"));
});

test("highlightSegments: empty text returns a single empty segment", () => {
  assert.deepEqual(highlightSegments("", ["foo"]), [{ text: "", match: false }]);
  assert.deepEqual(highlightSegments(null, ["foo"]), [{ text: "", match: false }]);
});

test("highlightSegments: no terms yields single non-match segment", () => {
  assert.deepEqual(
    highlightSegments("hello world", []),
    [{ text: "hello world", match: false }],
  );
});

test("highlightSegments: single match in middle", () => {
  assert.deepEqual(
    highlightSegments("auth fix", ["fix"]),
    [
      { text: "auth ", match: false },
      { text: "fix", match: true },
    ],
  );
});

test("highlightSegments: match at start and end", () => {
  assert.deepEqual(
    highlightSegments("foo bar", ["foo", "bar"]),
    [
      { text: "foo", match: true },
      { text: " ", match: false },
      { text: "bar", match: true },
    ],
  );
});

test("highlightSegments: case-insensitive but preserves original case", () => {
  assert.deepEqual(
    highlightSegments("Auth", ["auth"]),
    [{ text: "Auth", match: true }],
  );
});

test("highlightSegments: regex meta-chars treated literally", () => {
  assert.deepEqual(
    highlightSegments("a.b", ["."]),
    [
      { text: "a", match: false },
      { text: ".", match: true },
      { text: "b", match: false },
    ],
  );
});

test("highlightSegments: Unicode terms", () => {
  assert.deepEqual(
    highlightSegments("Привет мир", ["мир"]),
    [
      { text: "Привет ", match: false },
      { text: "мир", match: true },
    ],
  );
});

test("segmentsFromRanges: empty ranges yield single segment", () => {
  assert.deepEqual(
    segmentsFromRanges("hello", []),
    [{ text: "hello", match: false }],
  );
});

test("segmentsFromRanges: one range in middle", () => {
  assert.deepEqual(
    segmentsFromRanges("auth fix", [[0, 4]]),
    [
      { text: "auth", match: true },
      { text: " fix", match: false },
    ],
  );
});

test("segmentsFromRanges: multiple non-overlapping ranges", () => {
  assert.deepEqual(
    segmentsFromRanges("auth bar bug", [[0, 4], [9, 12]]),
    [
      { text: "auth", match: true },
      { text: " bar ", match: false },
      { text: "bug", match: true },
    ],
  );
});

test("segmentsFromRanges: handles char offsets, not byte offsets", () => {
  assert.deepEqual(
    segmentsFromRanges("Привет мир", [[7, 10]]),
    [
      { text: "Привет ", match: false },
      { text: "мир", match: true },
    ],
  );
});

test("segmentsFromRanges: clamps and skips out-of-range entries", () => {
  assert.deepEqual(
    segmentsFromRanges("abc", [[1, 100]]),
    [
      { text: "a", match: false },
      { text: "bc", match: true },
    ],
  );
  assert.deepEqual(
    segmentsFromRanges("abc", [[5, 10]]),
    [{ text: "abc", match: false }],
  );
});
