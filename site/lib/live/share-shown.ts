// The examples /play shows, by slug, for the share functions: written by the page's loader (app/routes/play.tsx), which drops an example the playground cannot type against the page's schema, so a shared ?example= previews only one the page opens. The share build, run after the page's, puts them in (vite.share.config.ts).
declare const __PLAY_EXAMPLES__: { slug: string; sql: string }[];

export const examples: ReadonlyMap<string, string> = new Map(__PLAY_EXAMPLES__.map((e) => [e.slug, e.sql]));
