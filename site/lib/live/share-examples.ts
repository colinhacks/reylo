// The playground's examples by their slugs, for the share functions: read from the main page's Inference section, which the bundle carries as text (vite.share.config.ts), as the build reads it.
import source from '../../content/sections/03-inference.mdx?raw';
import { inferenceStatements, START } from './inference.ts';

export const examples: ReadonlyMap<string, string> = new Map(inferenceStatements(source).map((e) => [e.slug, e.sql]));

/** The statement /play opens with, whose card is the page's own (app/routes/play-og.tsx). */
export const startSql = examples.get(START);
