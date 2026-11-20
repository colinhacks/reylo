#!/usr/bin/env node
// Checks a built docs site for dead links: every link on every page to another page of the site, and
// every heading it names, must exist. Run after the build, on its output:
//
//   node docs-kit/src/check-links.mjs build/client --site https://example.dev [--redirect-anchors /] [--ignore /api/]
//
// - It reads every .html file under the directory, and from each the ids it carries (`id="…"`) and the
//   links it makes (`href="…"`).
// - A link counts when it starts with `/` or `#`, or with the `--site` origin. The origin's links are
//   read out of the page's text too, so a URL written in a code block (an agent's instructions, say)
//   is checked like a link.
// - A path resolves to `<dir><path>/index.html`, `<dir><path>.html`, or a file at `<dir><path>` (a
//   Markdown copy, an image). A `#heading` on an HTML page must be one of its ids.
// - `--redirect-anchors <path>` (repeatable) names a page whose own script sends an unknown
//   `#heading` elsewhere, as a site that moved its sections does: its anchors aren't checked.
// - `--ignore <prefix>` (repeatable) skips the paths under it, which a server answers rather than a
//   file of the build (an API).
// It prints each dead link with the pages it's on, and exits 1 if there is one.

import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';

const args = process.argv.slice(2);
const dir = args.find((a, i) => !a.startsWith('--') && !['--site', '--redirect-anchors', '--ignore'].includes(args[i - 1]));
const siteAt = args.indexOf('--site');
const site = siteAt >= 0 && args[siteAt + 1] ? args[siteAt + 1].replace(/\/$/, '') : null;
const redirecting = new Set(args.flatMap((a, i) => (args[i - 1] === '--redirect-anchors' ? [a] : [])));
const ignored = args.flatMap((a, i) => (args[i - 1] === '--ignore' ? [a] : []));
if (!dir) {
  console.error('usage: check-links.mjs <build dir> [--site https://example.dev] [--redirect-anchors /] [--ignore /api/]');
  process.exit(2);
}
const root = path.resolve(dir);

/** Every .html file under the build, by the path a link names it with. */
const pages = new Map();
const walk = (at) => {
  for (const name of readdirSync(at)) {
    const file = path.join(at, name);
    if (statSync(file).isDirectory()) walk(file);
    else if (name.endsWith('.html')) {
      const rel = '/' + path.relative(root, file).split(path.sep).join('/');
      const route = rel.endsWith('/index.html') ? rel.slice(0, -'index.html'.length).replace(/(.)\/$/, '$1') : rel.slice(0, -'.html'.length);
      pages.set(route, readFileSync(file, 'utf8'));
    }
  }
};
walk(root);

/** The ids each page carries. */
const ids = new Map([...pages].map(([route, html]) => [route, new Set([...html.matchAll(/\sid="([^"]+)"/g)].map((m) => decode(m[1])))]));

function decode(s) {
  return s.replace(/&amp;/g, '&').replace(/&quot;/g, '"').replace(/&#x27;|&#39;/g, "'").replace(/&lt;/g, '<').replace(/&gt;/g, '>');
}

/** A link as a path and a heading, or null when it leaves the site. */
function target(href, from) {
  let url = decode(href.trim());
  if (site && url.startsWith(site)) url = url.slice(site.length) || '/';
  if (url.startsWith('#')) url = from + url;
  if (!url.startsWith('/') || url.startsWith('//')) return null;
  const [beforeHash, hash = ''] = url.split('#');
  const pathname = beforeHash.split('?')[0].replace(/(.)\/$/, '$1') || '/';
  return { pathname, hash: decodeURIComponent(hash) };
}

/** Whether a path is a page or a file of the build, and the page's route if it's a page. */
function resolve(pathname) {
  if (pages.has(pathname)) return { page: pathname };
  const file = path.join(root, pathname);
  if (file.startsWith(root) && existsSync(file) && statSync(file).isFile()) return { file: true };
  return null;
}

const dead = new Map();
const note = (link, from) => dead.set(link, [...(dead.get(link) ?? []), from]);

for (const [from, html] of pages) {
  const links = [...html.matchAll(/\shref="([^"]*)"/g)].map((m) => m[1]);
  // The site's own URLs written as text, in a code block or a sentence, are links too.
  if (site) {
    // The page's text, without its scripts and styles (a hydration payload repeats the text, escaped).
    const text = decode(html.replace(/<(script|style)\b[\s\S]*?<\/\1>/gi, '').replace(/<[^>]+>/g, ''));
    const own = new RegExp(`${site.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}(/[^\\s"'<>)\\]\`,]*)?`, 'g');
    for (const m of text.matchAll(own)) links.push(m[0].replace(/[.;:]$/, ''));
  }
  for (const href of new Set(links)) {
    const t = target(href, from);
    if (!t || ignored.some((prefix) => t.pathname.startsWith(prefix))) continue;
    const found = resolve(t.pathname);
    if (!found) note(href, from);
    else if (t.hash && found.page && !redirecting.has(found.page) && !ids.get(found.page).has(t.hash)) note(href, from);
  }
}

if (dead.size) {
  console.error(`${dead.size} dead link${dead.size === 1 ? '' : 's'}:`);
  for (const [link, from] of [...dead].sort()) console.error(`  ${link}  (on ${[...new Set(from)].sort().join(', ')})`);
  process.exit(1);
}
console.log(`check-links: ${pages.size} pages, no dead links`);
