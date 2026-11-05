import type { LanguageRegistration } from 'shiki';

/** A command, as the GitHub themes colour a shell's: the program, then its arguments. */
const command = [
  { match: '(&&|\\|\\||;)\\s*(\\S+)', captures: { 1: { name: 'keyword.operator' }, 2: { name: 'entity.name.function' } } },
  { match: '\\$[A-Za-z_][A-Za-z0-9_]*', name: 'variable.other' },
  { match: '\\\\$', name: 'keyword.operator' },
  { match: '\\G(?!--)\\S+', name: 'entity.name.function' },
  { match: '\\S+', name: 'string.unquoted.argument' },
];

/**
 * A ```terminal block: a session at a prompt, as the reader would see it. A line that opens with
 * `$ ` is a command, after a dim prompt; a line that opens with `# ` is a
 * comment; every other line is what the command printed, coloured as `reylo` paints it in a
 * terminal (engine/migrate/src/commands.rs): its marks (✓ green, ↺ and ! yellow, ✗ and its heading
 * red), the dim headings and timings, the statements `sync` ran and the commands a refusal names.
 * From a `Usage:` line on, it is help, as `reylo migrate --help` paints it (engine/cli/src/main.rs):
 * bold headings, commands, flags and the dim values they take.
 * The scopes are ones the GitHub themes colour without a background.
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
        { match: '^(\\s+)(--?[a-z-]+(?:, --[a-z-]+)?)( <\\S+)?(?=\\s{2})', captures: { 2: { name: 'variable.flag' }, 3: { name: 'comment.placeholder' } } },
        { match: '^(\\s+)([a-z][a-z-]*)( <\\S+)?(?=\\s{2})', captures: { 2: { name: 'support.function.command' }, 3: { name: 'comment.placeholder' } } },
      ],
    },
    { begin: '^(\\$) ', beginCaptures: { 1: { name: 'comment.prompt' } }, end: '$', patterns: command },
    { match: '^#.*$', name: 'comment.line' },
    // A file tree: its branches, and what follows a name.
    { match: '^[│\\s]*[├└]── .*?(?=\\s+#|$)', name: 'string.path' },
    { match: '(?<=\\s)# .*$', name: 'comment.line' },
    { match: '^✗ .*$', name: 'keyword.error' },
    { match: '^(Applying to|Syncing|Pulling from|Watching) .*$', name: 'comment.heading' },
    { match: '^(Nothing was (applied|changed)|No migration was written)\\.$', name: 'comment.outcome' },
    { match: '^Not run on this database.*$', name: 'comment.outcome' },
    { match: '^(\\s*)(✓)', captures: { 2: { name: 'entity.name.tag.ok' } } },
    { match: '^(\\s*)([↺!↓])', captures: { 2: { name: 'variable.warn' } } },
    { match: '^\\s+Dropped .*$', name: 'variable.warn' },
    // A migration recorded or taken out of the history, by name; then a statement `sync` ran.
    { match: '^(\\s+)(\\+) (?=\\S+\\.sql )', captures: { 2: { name: 'entity.name.tag.ok' } } },
    { match: '^(\\s+)(-) (?=\\S+\\.sql )', captures: { 2: { name: 'keyword.error' } } },
    { match: '^(\\s+)(\\+) (.*)$', captures: { 2: { name: 'entity.name.tag.ok' }, 3: { patterns: [{ include: 'source.sql' }] } } },
    { match: '^(\\s+)(-) (.*)$', captures: { 2: { name: 'keyword.error' }, 3: { patterns: [{ include: 'source.sql' }] } } },
    // A command a refusal names, and the lines a backslash continues it on.
    { begin: '^\\s{4,}(?=reylo |--)', end: '$', patterns: command },
    { match: '· \\d+ ms$', name: 'comment.timing' },
  ],
};
