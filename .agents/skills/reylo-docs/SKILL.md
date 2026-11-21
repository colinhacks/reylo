---
name: reylo-docs
description: How Reylo's docs, homepage and landing pages are written and laid out. Load before any change under site/ (content/, components/, the homepage or the Migrate landing), before writing a docs sentence anywhere else in the repository (a README, the skill in skills/), and again after a context compaction. Covers the sentence shape the docs hold to, measured against zod.dev; how a page and a section open; the rhythm of sentence and example; what never goes in (walkthroughs, pointer sentences, filler, reassurance); examples that are real and show the product's power; code blocks, tabs and their heights; headings; interactive UI that never moves by itself; and the checks to run before a change lands.
---

# Writing Reylo's docs

The docs are for a reader who skims. They read a short sentence, look at the example, and move on. Every rule below serves that reader: each sentence carries one fact the page doesn't already show, each example shows the thing working, and nothing makes them stop and parse.

The model is [zod.dev](https://zod.dev): one short sentence, then the example.

## Sentences

**Short, with one idea each.** Zod's docs have a median sentence of 11 words, and nine in ten under 20. Reylo's are held to the same. A sentence with two clauses, a parenthetical and a trailing qualifier is three sentences that haven't been split yet: split it at its joints.

| Instead of | Write |
| --- | --- |
| Name a parameter with a colon, and Reylo infers its type from where it's used, and compiles it to your database's own placeholder. | Name a parameter with a colon. Reylo infers its type from where it's used, and compiles it to your database's own placeholder. |
| Hyperdrive refuses prepared statements, so Reylo runs `mysql2` there through `query()` ([`mysql2`](/mysql#mysql2)). | Hyperdrive refuses prepared statements. So Reylo runs [`mysql2`](/mysql#mysql2) there through `query()`. |
| These are typed `boolean`: comparisons, `IN`, `BETWEEN`, `LIKE`, … and a `CASE` whose every value is one of them. | A sentence, then the list as a code block or table. |

- **A link goes on the words**, never in a parenthetical pointer: "[run your migrations on it](/deployment#build-before-migrate) before the build", not "… before the build ([Deployment](/deployment))".
- **An enumeration leaves the sentence.** Three or more code tokens in a row become a list, a table or a code block.
- **Imperative and direct.** "Hover over a query to see its types." Not "When you hover over a query, its types are shown."
- **Never open a sentence with inline code**, or with a lowercase word. Lead with a prose word: "The `--watch` flag pushes again on each save."
- **Plain words.** Reylo *infers* a type, never proves one. How many rows a query returns is its *cardinality*. A method is written `.run()`, never a bare `run`.

## What never goes in

Cut each of these on sight. None carries a fact.

- **A walkthrough of an example.** No "In this example:", no "Here, …", no "Notice that…", no sentence after a block that says what the block shows. The reader looks at an example for three seconds and sees what it does.
- **A pointer sentence** that only sends the reader elsewhere: "[Usage](/typescript) covers the setup." Link the words inline, or drop it.
- **An opener that restates the heading or the title**: "This page covers…", "This section explains…".
- **A filler clause**: "edit `schema.sql`, which holds all of it", "To change your schema, …".
- **Reassurance and defence**: "X keeps working", "this is safe", "rather than silently misread". The absence of a warning is the claim. Name a behaviour only when the reader must act differently because of it.
- **A distinction the reader can't know yet**: "Install Reylo, or the `reylo` command alone" asks them to understand two products before they've used one. Show the install, each way as a tab.
- **The exceptions inventory.** A reader needs the rule, not every place it bends. Keep an exception only where the reader will hit it, in the section about that thing.
- **A made-up category noun** the reader can't define, and a connector (*instead*, *rather*, *the same*) with no second term.

## A page, and a section

- **A page opens with one short sentence**, alone. Never go from the title straight into an h2: something of the page's own goes between.
- **A section opens with one short sentence.** The first paragraph is two sentences at most; what more it needs follows the example.
- **Never one h3 under an h2**, or one h4 under an h3. Two subsections or none: promote the one, or fold it into the parent.
- **Sibling headings share one form.** At one level of one section, all gerunds ("Trying a change", "Dropping and renaming", "Keeping a change made by hand") or all noun phrases, never a mix.
- **A command, flag or file is its own heading** where a section is its reference: `### reylo migrate new`. A file a prose heading is about sits at the heading's right as an aside chip: `## Schema <code className="dk-aside">schema.sql</code>`.
- **Order by the reader's task.** What they need first (get it working), then each capability with its example, then the alternatives, then internals. On a landing page, follow the workflow: model the schema, iterate, generate a migration, ship it.

## Rhythm

- **Sentence, then example.** The default beat is one or two sentences and then a block. A passage that runs past two sentences without reaching a block is the smell.
- **Never two blocks in a row.** A code block, table, list, callout, card or disclosure is never followed directly by another: a short sentence goes between, even one that only names what comes next ("Push it:", "Analyze it:"). Consecutive tabs are one block. Several `schema.sql` parts in one disclosure are one block.
- **Never three paragraphs in a row**, and at most one dense one between blocks. Break a run with a list, a table, a code block or a callout; never by shortening the paragraphs.
- **A callout is for one real heads-up**, a sentence or two.

## Examples

- **Real output only.** Every type, error and terminal line on a page is captured from the binary, never typed by hand: `nub run capture` for types and errors, a real run for terminal output. A command or flag that doesn't exist is never shown.
- **Realistic and calm.** Pick the case a reader actually has: switching from `feature-a` to `feature-b`, not back to `main`. Name things so they can't be mistaken for syntax: `rename_bio` and `tidy_columns`, never `about` or `tidy`.
- **Show the power where the point is power.** An example under "the most advanced diff engine" must be one a lesser engine gets wrong: a view over a retyped column, a constraint added without blocking writes, a SQLite rebuild that keeps the rows of the tables that reference it. A one-line `alter table` sells nothing.
- **Start simple, then build.** The first example of a page or a landing is the plainest one that works. The sophisticated ones come after the reader has the shape.
- **The minimum that makes its point.** A config example shows the key it's about. Every extra key is a question the reader has to answer.
- **Alternatives are tabs**, never stacked blocks: install methods, package managers, drivers, dialects. Write them with `tab="…"` on each fence.
- **The plain command, then the flag**, in one block: `reylo db push`, then `reylo db push --watch`.
- **Postgres first.** A dialect switch starts on Postgres and changes only when the reader picks another.

## Code blocks and cards

- **A long block scrolls.** A block that would run past a screenful, such as a JSON analysis, takes a max height and scrolls inside itself. A first example is small enough that it never scrolls.
- **Hover, don't link away.** Where a reader may want the definition behind a name, give it a dotted underline and show it on hover, as Twoslash does: a table's `create table` behind its name in a query. A page-level schema sits behind one "View schema" hover, or in a collapsed disclosure.
- **One card height.** Cards a reader flips between, such as the homepage's examples, are as tall as what sits beside them, and never taller: no card pads to its tallest sibling while the list beside it ends far above.

## Interactive UI

- **Nothing moves by itself.** No tab, dialect or example turns on a timer. No script reads the scroll position to move, hide or restyle anything; an element that should travel with the page and stay is `position: sticky`.
- **Every new or changed UI gets an optical pass** before anyone sees it: icons against their labels, labels against their buttons, measured from the ink, not the boxes. A chip, a code token or a badge looks the same wherever it appears: the table of contents, the page list, the pager and the prose.
- **A design that doesn't exist yet is a page, not a screenshot.** Mock it as one self-contained HTML file, every image inside it as a `data:` URI.
- **Light and dark both**, and a phone.

## Copy someone hands you

Copy written by hand for a page goes in as written. Fix only a wrong fact or a plain typo, each the smallest swap, and say which. Never "improve" it, and never offer its sentences up for cutting.

## What ships when

reylo.dev serves the `release` branch, which each release moves to its tag, so the live docs describe what npm serves. A docs change for a released feature can go live before the next release by a cherry-pick onto `release`; one about anything unreleased waits for the release.

## Before a change lands

- `nub run build` in `site/`. It type-checks, fails on a stale capture, and ends with `check-links` (no dead link, in a link or in text) and `check-headings` (no lone subsection, no title straight into an h2).
- Measure the sentences you touched against the bar above with `scripts/sentences.py`, beside this file:

  ```sh
  python3 .agents/skills/reylo-docs/scripts/sentences.py flag site/content/docs/<page>.mdx
  ```

  It prints every sentence over 20 words or with three or more joints (commas, colons, semicolons, a parenthesis), with its line.
- Read the rendered page, not the source: a component such as `<Analyzed>` or a tab group can put two blocks side by side that the source doesn't show.
- Screenshot every section you changed, desktop and phone, dark and light, and look at it.
