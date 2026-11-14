import { useEffect, useMemo, useState, type ComponentType } from 'react';
import * as runtime from 'react/jsx-runtime';
import { useSearchParams } from 'react-router';
import { InferenceCard } from '@/components/inference-card';
import { getMDXComponents } from '@/components/mdx';

/*
 * The local playground, at /playground on the dev server only (app/routes.ts): an inference card for
 * the statement in the address, drawn by the page's own components from the types Reylo infers for it
 * on the page's schema (lib/playground.ts, through lib/playground-plugin.ts).
 *
 *   ?q=<statement>                the statement (the editor writes it back as you type)
 *   &dialect=postgres|mysql|sqlite the dialect the card shows first
 *   &dialects=postgres,sqlite     the dialects to try it on (all three by default)
 *   &schema=<sql>                 a schema of the card's own, shown in its Schema row
 *   &theme=dark|light             the theme (the site's own otherwise, as the system's)
 *   &shot=1                       the card alone, for a screenshot
 */

type Card = { code?: string; errors: Record<string, string>; failure?: string };

const SAMPLE = `select u.name,
  jsonb_agg(jsonb_build_object('id', p.id, 'title', p.title)) as posts
from users u
join posts p on p.user_id = u.id
group by u.id`;

const NAMES: Record<string, string> = { postgres: 'Postgres', mysql: 'MySQL', sqlite: 'SQLite' };

export default function Playground() {
  const [params, setParams] = useSearchParams();
  const q = params.get('q') ?? SAMPLE;
  const dialects = params.get('dialects') ?? '';
  const schema = params.get('schema') ?? '';
  const shot = params.get('shot') === '1';
  const [text, setText] = useState(q);
  const [card, setCard] = useState<Card | null>(null);

  // The dialect the card shows first: the page's saved choice, which every dialect strip reads.
  useMemo(() => {
    const dialect = NAMES[params.get('dialect') ?? ''];
    if (dialect && typeof window !== 'undefined') {
      localStorage.setItem('dialect', dialect);
      sessionStorage.setItem('dialect', dialect);
    }
  }, [params]);

  // The theme the address names, saved as the site's theme switch saves it; otherwise the site's own.
  useEffect(() => {
    const theme = params.get('theme');
    if (theme !== 'dark' && theme !== 'light') return;
    localStorage.setItem('theme', theme);
    document.documentElement.classList.toggle('dark', theme === 'dark');
    document.documentElement.classList.toggle('light', theme === 'light');
    document.documentElement.style.colorScheme = theme;
  }, [params]);

  useEffect(() => {
    let current = true;
    setCard(null);
    fetch(`/__playground?${new URLSearchParams({ q, dialects, schema })}`)
      .then((r) => r.json() as Promise<Card>)
      .then((c) => current && setCard(c))
      .catch((e) => current && setCard({ errors: {}, failure: String(e) }));
    return () => {
      current = false;
    };
  }, [q, dialects, schema]);

  // The editor writes the statement back into the address a moment after the last keystroke.
  useEffect(() => {
    if (text === q) return;
    const timer = setTimeout(() => {
      const next = new URLSearchParams(params);
      next.set('q', text);
      setParams(next, { replace: true });
    }, 400);
    return () => clearTimeout(timer);
  }, [text]);

  const Content = useMemo(() => {
    if (!card?.code) return null;
    return (new Function(card.code)({ ...runtime, useMDXComponents: getMDXComponents }) as { default: ComponentType<{ components: unknown }> }).default;
  }, [card]);

  return (
    <div style={{ maxWidth: '50rem', margin: '0 auto', padding: shot ? '1.5rem' : '2rem 1.5rem' }}>
      {!shot && (
        <div style={{ marginBottom: '1.5rem', display: 'grid', gap: '0.5rem' }}>
          <textarea
            value={text}
            onChange={(e) => setText(e.target.value)}
            spellCheck={false}
            rows={Math.max(5, text.split('\n').length + 1)}
            style={{ width: '100%', font: '13px/1.5 ui-monospace, monospace', padding: '0.75rem', border: '1px solid var(--color-fd-border)', borderRadius: '0.5rem', background: 'var(--color-fd-card)', color: 'inherit' }}
          />
          <div style={{ fontSize: '0.8rem', color: 'var(--color-fd-muted-foreground)' }}>
            Add <code>&amp;shot=1</code> to the address for the card alone, <code>&amp;dialect=sqlite</code> to show another dialect first, <code>&amp;theme=light</code> or <code>&amp;theme=dark</code> for a theme, <code>&amp;schema=…</code> for a schema of the card&apos;s own.
          </div>
        </div>
      )}
      <div id="nd-page">
        <div className="prose">
          {Content ? <Content components={getMDXComponents({ PlaygroundCard: InferenceCard })} /> : !card ? <p>Analyzing…</p> : null}
        </div>
      </div>
      {card?.failure && <pre style={{ color: 'var(--color-fd-error, #c00)', whiteSpace: 'pre-wrap' }}>{card.failure}</pre>}
      {card && !shot && Object.keys(card.errors).length > 0 && (
        <div style={{ marginTop: '1rem', fontSize: '0.85rem' }}>
          {Object.entries(card.errors).map(([dialect, message]) => (
            <p key={dialect} style={{ margin: '0.25rem 0', color: 'var(--color-fd-muted-foreground)' }}>
              <strong>{NAMES[dialect] ?? dialect}:</strong> {message}
            </p>
          ))}
        </div>
      )}
    </div>
  );
}
