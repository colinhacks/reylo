// The playground's editor: CodeMirror with Postgres's SQL grammar, completing what fits where the
// cursor is (lib/live/complete.ts), closing and matching brackets, drawn as
// the page draws a code block. Shiki colours the text (lib/live/highlight.ts), and a diagnostic is
// drawn as the page draws every Reylo error, a squiggle under its range and the pinned card under
// its last line (docs-kit's `.dk-error` and `.dk-error-line`), in space the card opens there, so it
// never covers the code. The schema card's editor (createSchemaEditor) is the same editor without
// completions, for the DDL of a schema of the reader's own.
import { acceptCompletion, autocompletion, closeBrackets, closeBracketsKeymap, completionKeymap, completionStatus, moveCompletionSelection, selectedCompletion } from '@codemirror/autocomplete';
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
import { PostgreSQL, sql } from '@codemirror/lang-sql';
import { bracketMatching } from '@codemirror/language';
import { Compartment, EditorSelection, EditorState, RangeSetBuilder, StateEffect, StateField, type Extension, type Range } from '@codemirror/state';
import { Decoration, EditorView, keymap, ViewPlugin, WidgetType, type DecorationSet } from '@codemirror/view';
import type { HighlighterCore } from 'shiki/core';
import type { Catalog } from '../../../src/play-catalog.ts';
import { sqlCompletions } from './complete.ts';
import { tokens } from './highlight.ts';

export type Diagnostic = { message: string; code: number; line: number; column: number; endLine: number; endColumn: number; /** What the card names as the error's source, `reylo(<code>)` when none is given. */ source?: string };

/** The card under a line: the message and its source, `reylo(<code>)`, its arrow at the range's column. */
class ErrorCard extends WidgetType {
  constructor(
    readonly column: number,
    readonly message: string,
    readonly source: string,
  ) {
    super();
  }
  eq(other: ErrorCard) {
    return other.column === this.column && other.message === this.message && other.source === this.source;
  }
  toDOM() {
    const line = document.createElement('div');
    line.className = 'twoslash-meta-line dk-error-line nd-copy-ignore';
    const indent = document.createElement('span');
    indent.textContent = ' '.repeat(this.column);
    const card = document.createElement('span');
    card.className = 'twoslash-popup-container dk-error-popup nd-copy-ignore';
    const arrow = document.createElement('div');
    arrow.className = 'twoslash-popup-arrow';
    const text = document.createElement('code');
    text.className = 'twoslash-popup-code dk-error-text';
    const message = document.createElement('span');
    message.className = 'dk-error-message';
    message.textContent = this.message;
    const source = document.createElement('span');
    source.className = 'dk-error-source';
    source.textContent = this.source;
    text.append(message, ' ', source);
    card.append(arrow, text);
    line.append(indent, card);
    return line;
  }
  ignoreEvent() {
    return false;
  }
}

const setDiagnostics = StateEffect.define<Diagnostic[]>();

/** The squiggles and the cards, mapped through edits until the next answer replaces them. */
const diagnosticsField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(decorations, tr) {
    let next = decorations.map(tr.changes);
    for (const effect of tr.effects) {
      if (!effect.is(setDiagnostics)) continue;
      const doc = tr.state.doc;
      const ranges: Range<Decoration>[] = [];
      const pos = (line: number, column: number) => {
        const l = doc.line(Math.min(Math.max(line, 1), doc.lines));
        return Math.min(l.from + column - 1, l.to);
      };
      for (const d of effect.value) {
        const from = pos(d.line, d.column);
        let to = pos(d.endLine, d.endColumn);
        if (to <= from) to = Math.min(from + 1, doc.line(Math.min(d.endLine, doc.lines)).to);
        if (to > from) ranges.push(Decoration.mark({ class: 'dk-error' }).range(from, to));
        const last = doc.line(Math.min(Math.max(d.endLine, 1), doc.lines));
        ranges.push(Decoration.widget({ widget: new ErrorCard(d.column - 1, d.message, d.source ?? `reylo(${d.code})`), block: true, side: 1 }).range(last.to));
      }
      next = Decoration.set(ranges, true);
    }
    return next;
  },
  provide: (field) => EditorView.decorations.from(field),
});

/** Shiki's colours as marks, worked out again for the whole statement on every change: a statement is a few lines. */
function shikiColours(highlighter: HighlighterCore): Extension {
  const build = (state: EditorState) => {
    const builder = new RangeSetBuilder<Decoration>();
    const lines = tokens(highlighter, state.doc.toString(), 'sql');
    lines.forEach((line, n) => {
      let at = state.doc.line(n + 1).from;
      for (const t of line) {
        if (t.style && t.content.length) builder.add(at, at + t.content.length, Decoration.mark({ attributes: { style: t.style } }));
        at += t.content.length;
      }
    });
    return builder.finish();
  };
  return StateField.define<DecorationSet>({
    create: build,
    update: (value, tr) => (tr.docChanged ? build(tr.state) : value),
    provide: (field) => EditorView.decorations.from(field),
  });
}

/**
 * Enter as a plain editor has it: a new line at the indentation of the line the cursor is on, and
 * no more. The SQL grammar's own indentation pushes every line after a statement's first two
 * spaces in, which reads as a stray space.
 */
function newlineKeepingIndent(view: EditorView): boolean {
  const { state } = view;
  const changes = state.changeByRange((range) => {
    const line = state.doc.lineAt(range.from);
    const indent = /^[ \t]*/.exec(line.text)![0].slice(0, range.from - line.from);
    const insert = `${state.lineBreak}${indent}`;
    return { changes: { from: range.from, to: range.to, insert }, range: EditorSelection.cursor(range.from + insert.length) };
  });
  view.dispatch(state.update(changes, { scrollIntoView: true, userEvent: 'input' }));
  return true;
}

/** Each line's own indent as `--indent`, so a line that wraps hangs two characters past it, as a code block's does. */
const lineIndents = StateField.define<DecorationSet>({
  create: (state) => indents(state),
  update: (value, tr) => (tr.docChanged ? indents(tr.state) : value),
  provide: (field) => EditorView.decorations.from(field),
});

function indents(state: EditorState): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  for (let n = 1; n <= state.doc.lines; n++) {
    const line = state.doc.line(n);
    const indent = /^ */.exec(line.text)![0].length;
    if (indent) builder.add(line.from, line.from, Decoration.line({ attributes: { style: `--indent: ${indent}ch` } }));
  }
  return builder.finish();
}

/**
 * The completion list hangs left by its padding, its item's and the item's icon with its gap (34px),
 * so each name starts under the letters typed, but never closer than 8px to the window's edge. It
 * is set once CodeMirror has placed the list, from the left it gave it.
 */
const hangList = ViewPlugin.fromClass(
  class {
    observer: MutationObserver;
    constructor(readonly view: EditorView) {
      this.observer = new MutationObserver(() => this.fit());
      this.observer.observe(view.dom, { subtree: true, childList: true, attributes: true, attributeFilter: ['style'] });
    }
    fit() {
      const list = this.view.dom.querySelector<HTMLElement>('.cm-tooltip-autocomplete');
      if (!list) return;
      const left = parseFloat(list.style.left);
      if (Number.isNaN(left)) return;
      const margin = `${-Math.min(34, Math.max(0, left - 8))}px`;
      if (list.style.marginLeft !== margin) list.style.marginLeft = margin;
    }
    destroy() {
      this.observer.disconnect();
    }
  },
);

/** Tab with the completions open: the chosen item, or the first. */
function takeCompletion(view: EditorView): boolean {
  if (completionStatus(view.state) !== 'active') return false;
  if (!selectedCompletion(view.state)) moveCompletionSelection(true)(view);
  return acceptCompletion(view);
}

export type Editor = { view: EditorView; setDiagnostics(diagnostics: Diagnostic[]): void; setText(text: string): void; completing(): boolean; setCatalog(catalog: Catalog): void };

export function createEditor(parent: HTMLElement, doc: string, highlighter: HighlighterCore, catalog: Catalog, onChange: (text: string) => void): Editor {
  // The completions, replaced when the reader's own schema replaces the one the page opened with.
  const completions = new Compartment();
  const complete = (catalog: Catalog) => autocompletion({ selectOnOpen: false, filterStrict: true, override: [sqlCompletions(catalog)] });
  const view = new EditorView({
    parent,
    state: EditorState.create({
      doc,
      extensions: [
        history(),
        sql({ dialect: PostgreSQL }),
        // Only what fits where the cursor is (lib/live/complete.ts), each with its kind's icon and
        // its type, matched from its start. The list opens with nothing chosen, so Enter is always a new line until the
        // arrows choose an item; Tab takes the first item, or the one chosen.
        completions.of(complete(catalog)),
        hangList,
        closeBrackets(),
        bracketMatching(),
        keymap.of([...closeBracketsKeymap, { key: 'Tab', run: takeCompletion }, ...completionKeymap, { key: 'Enter', run: newlineKeepingIndent }, ...defaultKeymap, ...historyKeymap, indentWithTab]),
        EditorView.lineWrapping,
        EditorView.contentAttributes.of({ 'aria-label': 'SQL query', spellcheck: 'false', autocapitalize: 'off', autocorrect: 'off' }),
        shikiColours(highlighter),
        lineIndents,
        diagnosticsField,
        EditorView.updateListener.of((update) => {
          if (update.docChanged) onChange(update.state.doc.toString());
        }),
      ],
    }),
  });
  return {
    view,
    setDiagnostics: (diagnostics) => view.dispatch({ effects: setDiagnostics.of(diagnostics) }),
    // Another example's text: one change the reader can undo, the cursor at its end.
    completing: () => completionStatus(view.state) !== null,
    setText: (text) => view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text }, selection: { anchor: text.length }, scrollIntoView: true }),
    setCatalog: (catalog) => view.dispatch({ effects: completions.reconfigure(complete(catalog)) }),
  };
}

export type SchemaEditor = { view: EditorView; text(): string; setDiagnostics(diagnostics: Diagnostic[]): void };

/** The schema card's editor: the DDL, coloured and indented as the query is, with Mod-Enter for `onApply`, and an error drawn at its line as the query's are. */
export function createSchemaEditor(parent: HTMLElement, doc: string, highlighter: HighlighterCore, onApply: (text: string) => void): SchemaEditor {
  const view = new EditorView({
    parent,
    state: EditorState.create({
      doc,
      extensions: [
        history(),
        sql({ dialect: PostgreSQL }),
        closeBrackets(),
        bracketMatching(),
        keymap.of([{ key: 'Mod-Enter', run: (v) => (onApply(v.state.doc.toString()), true) }, ...closeBracketsKeymap, { key: 'Enter', run: newlineKeepingIndent }, ...defaultKeymap, ...historyKeymap, indentWithTab]),
        EditorView.lineWrapping,
        EditorView.contentAttributes.of({ 'aria-label': 'Schema DDL', spellcheck: 'false', autocapitalize: 'off', autocorrect: 'off' }),
        shikiColours(highlighter),
        lineIndents,
        diagnosticsField,
      ],
    }),
  });
  return {
    view,
    text: () => view.state.doc.toString(),
    setDiagnostics: (diagnostics) => view.dispatch({ effects: setDiagnostics.of(diagnostics) }),
  };
}
