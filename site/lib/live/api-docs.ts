// The playground's API, described once: the sections the API dialog draws (lib/live/api-dialog.tsx), and the same as Markdown, which the dialog's "Copy all" copies and /play/api.md and /llms.txt serve to agents (app/routes/play-api-md.ts). Free of Node and of React.
import type { PlayResult } from '../../../src/play-result.ts';

export type ApiDocs = {
  /** The analyze endpoint's address. */
  analyze: string;
  /** The schema endpoint's address. */
  schema: string;
  /** The page's own address, where it answers `?q=` with JSON when asked for JSON. */
  page?: string;
  /** What a query is analyzed against: "the playground's sample schema of users, posts and events". */
  against: string;
  /** The query of the examples. */
  example: string;
  /** The example's answer where the page already has it (the docs' build analyzes it, lib/live/play-data.ts), so the dialog shows it at once; else the dialog asks the API. */
  answer?: string;
};

/** reylo.dev's API (lib/live/api.ts), which the docs' /play describes, and a local copy of the site too, which does not serve it. */
export const REYLO_API: ApiDocs = {
  analyze: 'https://reylo.dev/play/api/analyze',
  schema: 'https://reylo.dev/play/api/schema',
  page: 'https://reylo.dev/play',
  against: "the playground's sample schema of users, posts and events",
  example: 'select name from users where id = :id',
};

export type ApiSection = { title: string; text: string; request: string; answer?: string };

/** An answer as the dialog shows it: without the fields that echo the request or time it. */
export function answerJson({ sql: _sql, ms: _ms, ...rest }: PlayResult): string {
  return JSON.stringify(rest, null, 2);
}

const shell = (s: string) => `'${s.replace(/'/g, `'\\''`)}'`;
const path = (url: string) => new URL(url).pathname;

/** The endpoints, each with one sentence and a request; the analyze endpoint's with `answer`, the example's answer, where there is one. */
export function apiSections(api: ApiDocs, answer?: string): ApiSection[] {
  // Encoded as a browser writes a form, a space as `+`, which reads more easily than `%20`.
  const q = `?${new URLSearchParams({ q: api.example })}`;
  return [
    ...(api.page ? [{ title: `GET ${path(api.page)}?q=<sql>`, text: "The playground's own link, asked for JSON, answers with the query's types, so a link you share works for an agent too.", request: `curl -H 'accept: application/json' ${shell(api.page + q)}` }] : []),
    {
      title: `GET ${path(api.analyze)}?q=<sql>`,
      text: `A query's params, return type, row type, cardinality and errors, analyzed against ${api.against} and never run.`,
      request: `curl ${shell(api.analyze + q)}`,
      ...(answer !== undefined ? { answer } : {}),
    },
    { title: `GET ${path(api.schema)}`, text: 'The tables with their columns and types, and the functions and type names the editor completes.', request: `curl ${shell(api.schema)}` },
  ];
}

/** The API as Markdown, for an agent. */
export function apiMarkdown(api: ApiDocs, answer?: string): string {
  const host = new URL(api.analyze).host;
  const intro = `Reylo infers a SQL query's TypeScript types from the database. The playground's API returns them as JSON, analyzed against ${api.against}; nothing is run.`;
  const sections = apiSections(api, answer).map((x) => [`## ${x.title}`, x.text, '```sh\n' + x.request + '\n```', ...(x.answer ? ['```json\n' + x.answer + '\n```'] : [])].join('\n\n'));
  return [`# ${host} playground API`, intro, ...sections].join('\n\n') + '\n';
}
