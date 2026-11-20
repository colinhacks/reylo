#!/usr/bin/env node
// Checks a built docs site's headings: no section holds exactly one subsection (an h2 with a lone
// h3 under it, an h3 with a lone h4), which reads as a stray in the table of contents. Run after
// the build, on its output:
//
//   node docs-kit/src/check-headings.mjs build/client
//
// It reads each page's article (`<article>`, else the whole page) and its h2 to h5 in order. It
// prints each lone subsection with its page, and exits 1 if there is one.

import { readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';

const dir = process.argv[2];
if (!dir) {
  console.error('usage: check-headings.mjs <build dir>');
  process.exit(2);
}
const root = path.resolve(dir);

const files = [];
const walk = (at) => {
  for (const name of readdirSync(at)) {
    const file = path.join(at, name);
    if (statSync(file).isDirectory()) walk(file);
    else if (name.endsWith('.html')) files.push(file);
  }
};
walk(root);

const text = (html) => html.replace(/<[^>]+>/g, '').replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&#x27;|&#39;/g, "'").trim();

const lone = [];
for (const file of files) {
  const html = readFileSync(file, 'utf8');
  const article = /<article\b[\s\S]*?<\/article>/.exec(html)?.[0] ?? html;
  const heads = [...article.matchAll(/<h([2-5])\b[^>]*>([\s\S]*?)<\/h\1>/g)].map((m) => ({ level: Number(m[1]), name: text(m[2]) }));
  heads.forEach((h, i) => {
    let children = 0;
    for (const next of heads.slice(i + 1)) {
      if (next.level <= h.level) break;
      if (next.level === h.level + 1) children++;
    }
    if (children === 1) lone.push(`${'/' + path.relative(root, file).split(path.sep).join('/')}: h${h.level} "${h.name}" has one h${h.level + 1}`);
  });
}

if (lone.length) {
  console.error(`${lone.length} section${lone.length === 1 ? '' : 's'} with a lone subsection:`);
  for (const line of lone) console.error(`  ${line}`);
  process.exit(1);
}
console.log(`check-headings: ${files.length} pages, no lone subsections`);
