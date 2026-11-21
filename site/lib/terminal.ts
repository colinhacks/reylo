import type { LanguageRegistration } from 'shiki';

/**
 * A ```terminal block: a session at a prompt, as the reader would see it. A line that opens with
 * `$ ` is a command, after a dim prompt; a line that opens with `# ` is a
 * comment; every other line is what the command printed, coloured as `reylo` paints it in a
 * terminal (engine/migrate/src/commands.rs): its marks (✓ green, ↺ and ! yellow, a » note dim whole, ✗ and its
 * heading red), the dim headings and timings, the statements `db push` ran and the commands a refusal names.
 * From a `Usage:` line on, it is help, as `reylo migrate --help` paints it (engine/cli/src/main.rs):
 * bold headings, commands, flags and the dim values they take.
 * The help's commands and flags, the commands a refusal names and the yellow marks take scopes of
 * their own (`reylo.cli.*`), coloured as the CLI paints them by `terminalColors` below; the rest are
 * scopes the GitHub themes colour without a background.
 */
export const terminal: LanguageRegistration = {
  name: 'terminal',
  scopeName: 'source.reylo-terminal',
  embeddedLangs: ['sql'],
  repository: {},
  patterns: [
    // A command's help, from its usage line to the end of the block.
    {
      begin: '^(Usage:) (.*)$',
      beginCaptures: { 1: { name: 'markup.bold' }, 2: { patterns: [{ match: '<[^>]+>|\\[[^\\]]+\\]', name: 'comment.placeholder' }] } },
      end: '(?!)',
      patterns: [
        { match: '^[A-Z][a-z]+:$', name: 'markup.bold' },
        { match: '^(\\s+)(--?[a-z-]+(?:, --[a-z-]+)?)( <\\S+)?(?=\\s{2})', captures: { 2: { name: 'reylo.cli.flag' }, 3: { name: 'comment.placeholder' } } },
        { match: '^(\\s+)([a-z][a-z -]*?)( <\\S+)?(?=\\s{2})', captures: { 2: { name: 'reylo.cli.command' }, 3: { name: 'comment.placeholder' } } },
      ],
    },
    // What the reader types, plain after a dim prompt, as a terminal shows it.
    { match: '^(\\$) ', captures: { 1: { name: 'comment.prompt' } } },
    { match: '^#.*$', name: 'comment.line' },
    // A file tree: its branches, and what follows a name.
    { match: '^[│\\s]*[├└]── .*?(?=\\s+#|$)', name: 'string.path' },
    { match: '(?<=\\s)# .*$', name: 'comment.line' },
    { match: '^✗ .*$', name: 'keyword.error' },
    { match: '^(Applying to|Pushing|Pulling from|Watching) .*$', name: 'comment.heading' },
    { match: '^(Nothing was (applied|changed)|No migration was written)\\.$', name: 'comment.outcome' },
    { match: '^Not run on this database.*$', name: 'comment.outcome' },
    { match: '^(\\s*)(✓)', captures: { 2: { name: 'entity.name.tag.ok' } } },
    { match: '^(\\s*)([↺!])', captures: { 2: { name: 'reylo.cli.warn' } } },
    { match: '^(\\s*)(↓)', captures: { 2: { name: 'entity.name.tag.ok' } } },
    // A note is dim whole, and so is each lead that names a next step, so that its commands stand out.
    { match: '^\\s*» .*$', name: 'comment.note' },
    { match: '^\\s+[A-Z][^✓✗!↺↓+·»]*:$', name: 'comment.note' },
    { match: '^\\s+Dropped .*$', name: 'reylo.cli.warn' },
    // A migration recorded or taken out of the history, by name; then a statement `db push` ran.
    { match: '^(\\s+)(\\+) (?=\\S+\\.sql )', captures: { 2: { name: 'entity.name.tag.ok' } } },
    { match: '^(\\s+)(-) (?=\\S+\\.sql )', captures: { 2: { name: 'keyword.error' } } },
    { match: '^(\\s+)(\\+) (.*)$', captures: { 2: { name: 'entity.name.tag.ok' }, 3: { patterns: [{ include: 'source.sql' }] } } },
    { match: '^(\\s+)(-) (.*)$', captures: { 2: { name: 'keyword.error' }, 3: { patterns: [{ include: 'source.sql' }] } } },
    // A command a refusal names, and the lines a backslash continues it on.
    { match: '^\\s{4,}(reylo |--).*$', name: 'reylo.cli.command' },
    { match: '· \\d+ ms$', name: 'comment.timing' },
  ],
};

/**
 * The colours the CLI paints with that the GitHub themes have no scope for: its cyan, for a command
 * the help or a refusal names, and its yellow, for a flag and for the ! and ↺ marks. Added to each
 * theme's token colours (lib/mdx-options.ts).
 */
export const terminalColors = {
  dark: [
    { scope: 'reylo.cli.command', settings: { foreground: '#6dd1db' } },
    { scope: ['reylo.cli.flag', 'reylo.cli.warn'], settings: { foreground: '#fbc40e' } },
  ],
  light: [
    { scope: 'reylo.cli.command', settings: { foreground: '#0e7c86' } },
    { scope: ['reylo.cli.flag', 'reylo.cli.warn'], settings: { foreground: '#9a6700' } },
  ],
};
