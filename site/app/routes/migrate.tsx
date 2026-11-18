import type { Route } from './+types/migrate';
import { MigrateLanding } from '@/components/migrate-landing';
import { siteUrl } from '@/lib/layout.shared';

// Reylo Migrate's landing page (components/migrate-landing.tsx); its docs are at /migrate/docs.
export const meta: Route.MetaFunction = () => {
  const title = 'Reylo Migrate — SQL-first database migrations for busy teams';
  const description = 'Keep your schema in one SQL file. Reylo Migrate writes each change as a migration of plain SQL, on Postgres, MySQL and SQLite.';
  return [
    { title },
    { name: 'description', content: description },
    { tagName: 'link', rel: 'canonical', href: `${siteUrl}/migrate` },
    { property: 'og:title', content: title },
    { property: 'og:description', content: description },
    { property: 'og:url', content: `${siteUrl}/migrate` },
    { property: 'og:site_name', content: 'Reylo' },
  ];
};

/**
 * Run at build time, when the page is rendered to a file (react-router.config.ts): Reylo's star
 * count, for the nav's GitHub link, or none when GitHub can't be reached.
 */
export async function loader() {
  try {
    const res = await fetch('https://api.github.com/repos/reylosql/reylo', { headers: { Accept: 'application/vnd.github+json' } });
    const repo = (await res.json()) as { stargazers_count?: number };
    return { stars: res.ok ? repo.stargazers_count : undefined };
  } catch {
    return { stars: undefined };
  }
}

export default function Migrate({ loaderData }: Route.ComponentProps) {
  return <MigrateLanding stars={loaderData.stars} />;
}
