import type { Route } from './+types/migrate';
import { MigrateLanding } from '@/components/migrate-landing';
import { siteUrl } from '@/lib/layout.shared';

// Reylo Migrate's landing page (components/migrate-landing.tsx); its docs are at /migrate/docs.
export const meta: Route.MetaFunction = () => {
  const title = 'Reylo Migrate: SQL-first database migrations';
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

export default function Migrate() {
  return <MigrateLanding />;
}
