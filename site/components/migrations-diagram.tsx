import type { ReactNode } from 'react';

// The site's theme variables, so the figure follows the light and dark themes as the text does.
const color = {
  text: 'var(--color-fd-foreground)',
  muted: 'var(--color-fd-muted-foreground)',
  headings: 'var(--color-fd-headings)',
  border: 'var(--color-fd-border)',
  card: 'var(--color-fd-card)',
  accent: 'var(--color-fd-primary)',
};
const mono = 'var(--font-code), ui-monospace, monospace';
const sans = 'var(--font-text), ui-sans-serif, system-ui, sans-serif';

type Without = 'schema' | 'lock';

const labels: Record<'full' | Without, string> = {
  full: 'How the migration commands move between schema.sql, the migrations and the database. reylo migrate new writes a migration from schema.sql into the migrations folder, whose reylo-lock.yaml records their order. reylo migrate dev and reylo migrate deploy run the migrations on the database. Off to the sides, skipping the migrations: reylo migrate init writes schema.sql from an existing database, and reylo db push changes the database straight from schema.sql.',
  schema: 'The same diagram without schema.sql: the file is deleted, so reylo migrate init and reylo db push, which touch it, are grayed out. reylo migrate new still writes the next migration, now from the database, up into the migrations folder, beside reylo migrate dev and reylo migrate deploy, which run the migrations on the database.',
  lock: 'The same diagram without the lock: reylo-lock.yaml is deleted and grayed out, and the migrations run in name order. Every command remains.',
};

// Geist Mono's advance is 0.6em, so a label's width is known without measuring it.
const LABEL = 12;
const textWidth = (s: string, size = LABEL) => s.length * size * 0.6;
const chipWidth = (s: string) => Math.ceil(textWidth(s) + 20);

const CARD_W = 208;
const CHIP_H = 24;
const commands = {
  new: 'reylo migrate new',
  run: 'reylo migrate {dev|deploy}',
  init: 'reylo migrate init',
  push: 'reylo db push',
};

const icons = {
  file: 'M4.5 1.75h4.75l3.25 3.25v8.5a.75.75 0 0 1-.75.75h-7.25a.75.75 0 0 1-.75-.75v-11a.75.75 0 0 1 .75-.75z M9.25 1.75v3.25h3.25',
  folder: 'M1.75 4.25a.75.75 0 0 1 .75-.75h3.6l1.5 1.5h5.9a.75.75 0 0 1 .75.75v6.5a.75.75 0 0 1-.75.75h-11a.75.75 0 0 1-.75-.75z',
  lock: 'M4 7.25h8a.75.75 0 0 1 .75.75v5a.75.75 0 0 1-.75.75h-8a.75.75 0 0 1-.75-.75v-5a.75.75 0 0 1 .75-.75z M5.5 7.25v-2a2.5 2.5 0 0 1 5 0v2',
  db: 'M2.75 4c0-1.1 2.35-2 5.25-2s5.25.9 5.25 2-2.35 2-5.25 2-5.25-.9-5.25-2z M2.75 4v8c0 1.1 2.35 2 5.25 2s5.25-.9 5.25-2v-8 M2.75 8c0 1.1 2.35 2 5.25 2s5.25-.9 5.25-2',
};

/** Something deleted in this mode: drawn faint, its outlines dashed. */
function Gone({ off, children }: { off: boolean; children: ReactNode }) {
  return <g style={off ? { opacity: 0.35 } : undefined}>{children}</g>;
}

type TextProps = { x: number; y: number; children: ReactNode; size?: number; fill?: string; weight?: number; anchor?: 'start' | 'middle' | 'end'; code?: boolean };

/** Real, selectable text in the page's own faces. */
function Text({ x, y, children, size = LABEL, fill = color.text, weight = 400, anchor = 'start', code = false }: TextProps) {
  return (
    <text x={x} y={y} textAnchor={anchor} style={{ fill, fontFamily: code ? mono : sans, fontSize: size, fontWeight: weight, whiteSpace: 'pre', fontVariantLigatures: code ? 'none' : undefined }}>
      {children}
    </text>
  );
}

function Icon({ x, y, d }: { x: number; y: number; d: string }) {
  return <path d={d} transform={`translate(${x} ${y}) scale(${14 / 16})`} style={{ fill: 'none', stroke: color.muted, strokeWidth: 1.45, strokeLinecap: 'round', strokeLinejoin: 'round' }} />;
}

/** A card's title row: an icon, then a file name in code (struck through when deleted) or a name. */
function Title({ x, y, icon, name, code = true, struck = false }: { x: number; y: number; icon: string; name: string; code?: boolean; struck?: boolean }) {
  return (
    <g>
      <Icon x={x + 14} y={y - 11} d={icon} />
      <Text x={x + 36} y={y} code={code} size={code ? 12 : 13} weight={code ? 500 : 600} fill={color.headings}>{name}</Text>
      {struck && <line x1={x + 34} x2={x + 38 + textWidth(name)} y1={y - 3.6} y2={y - 3.6} style={{ stroke: color.muted, strokeWidth: 1.3 }} />}
    </g>
  );
}

function Card({ x, y, h, dashed = false }: { x: number; y: number; h: number; dashed?: boolean }) {
  return <rect x={x + 0.5} y={y + 0.5} width={CARD_W - 1} height={h - 1} rx={10} style={{ fill: color.card, stroke: color.border, strokeDasharray: dashed ? '4 3' : undefined }} />;
}

/** A line through `points` with rounded corners and an arrowhead at its end, or none with `head={false}`. */
function Line({ points, accent = false, dashed = false, head = true }: { points: [number, number][]; accent?: boolean; dashed?: boolean; head?: boolean }) {
  const stroke = accent ? color.accent : color.muted;
  const width = accent ? 1.75 : 1.25;
  const L = accent ? 8 : 7, W = accent ? 4.25 : 3.75, r = 10;
  const p = points.map((q) => [...q] as [number, number]);
  const n = p.length;
  const [ax, ay] = p[n - 2], [bx, by] = p[n - 1];
  const len = Math.hypot(bx - ax, by - ay), ux = (bx - ax) / len, uy = (by - ay) / len;
  if (head) p[n - 1] = [bx - ux * (L - 0.5), by - uy * (L - 0.5)];
  let d = `M${p[0][0]} ${p[0][1]}`;
  for (let i = 1; i < n - 1; i++) {
    const [x0, y0] = p[i - 1], [x1, y1] = p[i], [x2, y2] = p[i + 1];
    const l1 = Math.hypot(x1 - x0, y1 - y0), l2 = Math.hypot(x2 - x1, y2 - y1);
    const rr = Math.min(r, l1 / 2, l2 / 2);
    d += ` L${x1 - ((x1 - x0) / l1) * rr} ${y1 - ((y1 - y0) / l1) * rr} Q${x1} ${y1} ${x1 + ((x2 - x1) / l2) * rr} ${y1 + ((y2 - y1) / l2) * rr}`;
  }
  d += ` L${p[n - 1][0]} ${p[n - 1][1]}`;
  const px = -uy, py = ux, base = [bx - ux * L, by - uy * L], notch = [bx - ux * L * 0.78, by - uy * L * 0.78];
  return (
    <g>
      <path d={d} style={{ fill: 'none', stroke, strokeWidth: width, strokeDasharray: dashed ? '4 3' : undefined }} />
      {head && <path d={`M${bx} ${by} L${base[0] + px * W} ${base[1] + py * W} L${notch[0]} ${notch[1]} L${base[0] - px * W} ${base[1] - py * W} Z`} style={{ fill: stroke, stroke, strokeWidth: 0.5, strokeLinejoin: 'round' }} />}
    </g>
  );
}

/** A command, set on its line: a chip centred at (cx, cy) that the line runs into and out of. */
function Chip({ cx, cy, command, accent = false, dashed = false }: { cx: number; cy: number; command: string; accent?: boolean; dashed?: boolean }) {
  const w = chipWidth(command);
  return (
    <g>
      <rect
        x={cx - w / 2 + 0.5}
        y={cy - CHIP_H / 2 + 0.5}
        width={w - 1}
        height={CHIP_H - 1}
        rx={7}
        style={{
          fill: accent ? `color-mix(in oklab, ${color.accent} 10%, ${color.card})` : color.card,
          stroke: accent ? `color-mix(in oklab, ${color.accent} 55%, transparent)` : `color-mix(in oklab, ${color.muted} 45%, transparent)`,
          strokeDasharray: dashed ? '4 3' : undefined,
        }}
      />
      <Text x={cx} y={cy + 4.2} anchor="middle" code weight={500} fill={accent ? color.accent : color.text}>{command}</Text>
    </g>
  );
}

/** A vertical line from y0 to y1 at x, broken by a chip at cy, with the arrowhead at y1. */
function Through({ x, y0, y1, cy, command, accent = false, dashed = false }: { x: number; y0: number; y1: number; cy: number; command: string; accent?: boolean; dashed?: boolean }) {
  const down = y1 > y0;
  const top = cy - CHIP_H / 2, bottom = cy + CHIP_H / 2;
  return (
    <g>
      <Line points={[[x, y0], [x, down ? top : bottom]]} accent={accent} dashed={dashed} head={false} />
      <Line points={[[x, down ? bottom : top], [x, y1]]} accent={accent} dashed={dashed} />
      <Chip cx={x} cy={cy} command={command} accent={accent} dashed={dashed} />
    </g>
  );
}

/**
 * The figure, for a column `form` wide: three cards stacked, `reylo migrate new` and
 * `reylo migrate {dev|deploy}` on the arrows between them, and the two commands that skip the
 * migrations on lines down the sides, `reylo migrate init` up the left and `reylo db push` down the
 * right. The wide form sets the side chips beside the migrations; the narrow one, for a phone, sets
 * each in a band of its own between the cards, so the figure keeps its text at its drawn size.
 */
function Figure({ without, narrow }: { without?: Without; narrow: boolean }) {
  const noSchema = without === 'schema', noLock = without === 'lock';
  const initChip = chipWidth(commands.init), pushChip = chipWidth(commands.push), runChip = chipWidth(commands.run), newChip = chipWidth(commands.new);

  // across: where the side lines run, and the cards' centre. Without schema.sql, reylo migrate new
  // runs up from the database beside {dev|deploy}, its arrow DX_NEW right of the centre and
  // {dev|deploy}'s DX_RUN left of it, each chip clear of the other's line; on a phone the side lines
  // move out to clear the pair, and their grayed chips sit inside them, between the first two cards.
  const DX_RUN = -40, DX_NEW = 70;
  let left: number, cx: number, right: number, W: number;
  if (narrow && noSchema) {
    const reach = Math.ceil(Math.max(-DX_RUN + runChip / 2, DX_NEW + newChip / 2) + 6);
    cx = 12 + reach;
    left = cx - reach;
    right = cx + reach;
    W = right + 12;
  } else if (narrow) {
    // the side lines clear the {dev|deploy} chip, and init's chip clears the new arrow
    const reach = Math.max(runChip / 2, CARD_W / 2) + 12;
    cx = Math.ceil(12 + initChip / 2 + reach);
    left = cx - reach;
    right = cx + reach;
    W = Math.ceil(right + pushChip / 2 + 12);
  } else {
    left = Math.ceil(12 + initChip / 2);
    cx = Math.ceil(left + initChip / 2 + 12 + CARD_W / 2);
    right = Math.ceil(cx + CARD_W / 2 + 12 + pushChip / 2);
    W = Math.ceil(right + pushChip / 2 + 12);
  }
  const x0 = cx - CARD_W / 2, x1 = cx + CARD_W / 2;

  // down: the cards, and between them one row of chips or two, a row 28px in from each card
  const gap1 = narrow ? 92 : 72, gap2 = narrow || noSchema ? 92 : 72;
  const ty = 12, th = 46, my = ty + th + gap1, mh = 86, by = my + mh + gap2, bh = 46;
  const H = by + bh + 12;
  const g1 = ty + th, g2 = my + mh;
  const newY = narrow ? g1 + gap1 - 28 : g1 + gap1 / 2;
  const runY = narrow || noSchema ? g2 + 28 : g2 + gap2 / 2;
  const newUpY = g2 + gap2 - 28;
  const initY = narrow ? g1 + 28 : my + mh / 2;
  const pushY = narrow ? (noSchema ? g1 + gap1 - 28 : g2 + gap2 - 28) : my + mh / 2;
  // the side chips: across their lines, or on a phone without schema.sql, inside them
  const inside = narrow && noSchema;
  const initX = inside ? left + 8 + initChip / 2 : left;
  const pushX = inside ? right - 8 - pushChip / 2 : right;
  const ys = ty + th / 2, yb = by + bh / 2;

  return (
    <svg viewBox={`0 0 ${W} ${H}`} role="img" aria-label={labels[without ?? 'full']} style={{ display: 'block', width: '100%', height: 'auto', maxWidth: W, marginInline: 'auto' }}>
      <title>{labels[without ?? 'full']}</title>

      <Gone off={noSchema}>
        <Card x={x0} y={ty} h={th} dashed={noSchema} />
        <Title x={x0} y={ty + 28} icon={icons.file} name="schema.sql" struck={noSchema} />
      </Gone>

      <Card x={x0} y={my} h={mh} />
      <Title x={x0} y={my + 27} icon={icons.folder} name="migrations/" />
      <line x1={x0} x2={x1} y1={my + 43.5} y2={my + 43.5} style={{ stroke: color.border }} />
      <Gone off={noLock}>
        {noLock && <rect x={x0 + 5.5} y={my + 48.5} width={CARD_W - 11} height={mh - 54} rx={6} style={{ fill: 'none', stroke: color.border, strokeDasharray: '4 3' }} />}
        <Title x={x0} y={my + 70} icon={icons.lock} name="reylo-lock.yaml" struck={noLock} />
      </Gone>

      <Card x={x0} y={by} h={bh} />
      <Title x={x0} y={by + 28} icon={icons.db} name="Database" code={false} />

      {noSchema ? (
        <>
          <Through x={cx + DX_RUN} y0={g2} y1={by} cy={runY} command={commands.run} accent />
          {/* new, from the database: what it has that the migrations don't */}
          <Through x={cx + DX_NEW} y0={by} y1={g2} cy={newUpY} command={commands.new} accent />
        </>
      ) : (
        <>
          <Through x={cx} y0={g1} y1={my} cy={newY} command={commands.new} accent />
          <Through x={cx} y0={g2} y1={by} cy={runY} command={commands.run} accent />
        </>
      )}

      <Gone off={noSchema}>
        {/* init, up the left: from a database to schema.sql */}
        {inside ? (
          <Line points={[[x0, yb], [left, yb], [left, ys], [x0, ys]]} dashed />
        ) : (
          <>
            <Line points={[[x0, yb], [left, yb], [left, initY + CHIP_H / 2]]} dashed={noSchema} head={false} />
            <Line points={[[left, initY - CHIP_H / 2], [left, ys], [x0, ys]]} dashed={noSchema} />
          </>
        )}
        <Chip cx={initX} cy={initY} command={commands.init} dashed={noSchema} />
        {/* db push, down the right: from schema.sql to the database */}
        {inside ? (
          <Line points={[[x1, ys], [right, ys], [right, yb], [x1, yb]]} dashed />
        ) : (
          <>
            <Line points={[[x1, ys], [right, ys], [right, pushY - CHIP_H / 2]]} dashed={noSchema} head={false} />
            <Line points={[[right, pushY + CHIP_H / 2], [right, yb], [x1, yb]]} dashed={noSchema} />
          </>
        )}
        <Chip cx={pushX} cy={pushY} command={commands.push} dashed={noSchema} />
      </Gone>
    </svg>
  );
}

/**
 * The diagram at the top of Migrations: schema.sql, the migrations and the database, and the
 * commands that move between them. `without` draws it with schema.sql or the lock deleted, the
 * parts that go with it grayed out, and `caption` says what it shows, under it. A container query picks the form: the wide one (520px) where
 * the column has room for it, and below that the narrow one (388px), which a phone's column shows
 * at no less than 92% of its size.
 */
export function MigrationsDiagram({ without, caption }: { without?: Without; caption?: string }) {
  return (
    <figure className="@container my-8">
      <div className="hidden @min-[520px]:block">
        <Figure without={without} narrow={false} />
      </div>
      <div className="@min-[520px]:hidden">
        <Figure without={without} narrow />
      </div>
      {caption ? <figcaption className="reylo-figure-caption">{caption}</figcaption> : null}
    </figure>
  );
}
