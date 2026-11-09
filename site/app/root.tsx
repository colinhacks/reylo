import './global.css';
import { DocsDocument } from 'docs-kit/document';
import type { ReactNode } from 'react';
import { isRouteErrorResponse, Outlet } from 'react-router';
import type { Route } from './+types/root';
import caveatLatin from '../assets/fonts/caveat-latin.woff2?url';
import plexLatin from '../assets/fonts/ibm-plex-sans-latin.woff2?url';
import geistMonoLatin from '../assets/fonts/geist-mono-latin.woff2?url';
import orbitron from '../assets/fonts/orbitron.woff2?url';
import fontsCss from './fonts.css?url';
import { NotFound } from '@/components/not-found';
import { DIALECTS } from '@/lib/dialects';

// The fonts (app/fonts.css): IBM Plex Sans for text, Geist Mono for code, and Caveat for the
// "Leave a star!" note, with the faces next/font gave them while the site was on Next.js:
// `font-display: optional` for IBM Plex Sans and Geist Mono (a font that arrived after the first paint
// rewrapped the opening paragraph, a layout shift of 0.057 at 1440 wide and 0.146 at 390, measured
// 2026-10-20), IBM Plex Sans's size-adjusted fallback, and the system's monospace behind Geist Mono. Their
// Latin files are preloaded, so they make the first paint on any ordinary connection; a first view
// whose fonts take longer keeps the fallbacks, and the next view has them cached. The faces are a
// stylesheet of their own, linked here rather than imported by global.css: under `vite dev`, React
// Router serves the imported CSS as a stylesheet that it removes once the page hydrates, when Vite
// injects its own copy, and the faces declared again then missed `optional`'s window, so the page
// went to Arial and Menlo for the rest of the visit (docs-kit's README). A linked
// stylesheet stays put.
export const links: Route.LinksFunction = () => [
  { rel: 'stylesheet', href: fontsCss },
  ...[plexLatin, geistMonoLatin, caveatLatin, orbitron].map((href) => ({ rel: 'preload', href, as: 'font', type: 'font/woff2', crossOrigin: 'anonymous' as const })),
  { rel: 'manifest', href: '/manifest.webmanifest' },
  { rel: 'apple-touch-icon', href: '/apple-icon.png', sizes: '180x180', type: 'image/png' },
  { rel: 'author', href: 'https://github.com/colinhacks' },
];

// The favicon follows the browser's theme: the black mark on a light tab, the white one on a dark
// tab. The SVG's own media query (public/icon.svg) does this in Chrome and Firefox, but Safari
// ignores it (WebKit bug 309949), and Chrome draws the icon once and keeps it when the theme
// changes. So this script adds the icon links itself, each naming its file for the theme (the ICO
// for browsers that read no SVG icon), and changes them when the theme changes, which makes Chrome
// read them again. It runs while <head> is parsed, before Safari reads the page's icons, which it
// does once, just before the load event, so Safari shows the icon of the theme the page loaded in.
// React never sees these links: React 19 matches the icon links it renders by their href, and puts
// back the original of any whose href a script has changed. The ICO is declared 48 pixels and the
// SVG any size, so Chrome takes the SVG (it scores `any` above a size it would have to scale). The
// icons follow the browser, not the site's theme switch, since the browser draws the tab.
const themeIcons = `try {
  const dark = matchMedia('(prefers-color-scheme: dark)');
  const icon = (sizes, type) => {
    const link = document.createElement('link');
    link.rel = 'icon';
    link.sizes = sizes;
    if (type) link.type = type;
    document.head.appendChild(link);
    return link;
  };
  const ico = icon('48x48');
  const svg = icon('any', 'image/svg+xml');
  const apply = () => {
    ico.href = dark.matches ? '/favicon-dark.ico' : '/favicon.ico';
    svg.href = dark.matches ? '/icon-dark.svg' : '/icon-light.svg';
  };
  apply();
  dark.addEventListener('change', apply);
} catch {}`;

// For a client that runs no script, such as a crawler: the black ICO and the SVG that colours
// itself by its own media query.
const staticIcons = '<link rel="icon" href="/favicon.ico" sizes="48x48"><link rel="icon" href="/icon.svg" type="image/svg+xml" sizes="any">';

// docs-kit's document: Fumadocs' provider, and the head scripts that show a saved tab choice from
// the first paint and keep a pressed tab under the pointer. Every dialect choice on the page is one
// tab group, `dialect` (components/dialect-tabs.tsx); a saved choice written in another case, such
// as the `postgres` an earlier version of the page saved, is put in today's spelling before the tabs
// read it, and any other value is dropped, since Fumadocs' tabs would otherwise select nothing.
export function Layout({ children }: { children: ReactNode }) {
  return (
    <DocsDocument
      tabs={{ dialect: DIALECTS }}
      head={
        <>
          <script dangerouslySetInnerHTML={{ __html: themeIcons }} />
          <noscript dangerouslySetInnerHTML={{ __html: staticIcons }} />
        </>
      }
    >
      {children}
    </DocsDocument>
  );
}

export default function App() {
  return <Outlet />;
}

// What a failed navigation shows: the missing-address page for a 404 (a link to a page the site
// does not have), and the status for anything else, with the error itself in `vite dev`.
export function ErrorBoundary({ error }: Route.ErrorBoundaryProps) {
  if (isRouteErrorResponse(error)) return <NotFound status={error.status} text={error.status === 404 ? undefined : error.statusText || 'This page could not be shown.'} />;
  return <NotFound status={500} text={import.meta.env.DEV ? String(error) : 'This page could not be shown.'} />;
}
