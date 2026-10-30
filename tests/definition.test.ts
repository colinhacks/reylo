import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, realpathSync, symlinkSync, writeFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { ROOT, languageServer } from "./helpers.ts";

// Go-to-definition from what a `.sql` module declares opens the `.sql` file at its statement, through
// the span map the content mapper returns (src/mapper.ts), and a hover shows the file's comment and
// statement, from the doc comments the module carries (src/doc-comment.ts). The language server
// runs as an editor runs it, `tsc --lsp --stdio` with runExternalCode on, over a SQLite project, so
// no server is needed.

type Position = { line: number; character: number };
type Range = { start: Position; end: Position };

function project(): string {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "reylo-definition-")));
  writeFileSync(join(root, "package.json"), `{ "type": "module" }`);
  writeFileSync(join(root, ".env"), "DATABASE_URL=file:./app.db\n");
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table posts (id integer primary key, title text not null, body text)");
  db.close();
  mkdirSync(join(root, "node_modules", "@reylosql"), { recursive: true });
  symlinkSync(ROOT, join(root, "node_modules", "reylo"));
  symlinkSync(join(ROOT, "packages", "sqlite"), join(root, "node_modules", "@reylosql", "sqlite"));
  const compilerOptions = { target: "esnext", module: "nodenext", lib: ["esnext"], types: [], strict: true, noEmit: true, skipLibCheck: true, allowImportingTsExtensions: true, paths: { "@/*": ["./*"] } };
  writeFileSync(join(root, "tsconfig.json"), JSON.stringify({ compilerOptions, contentMappers: [{ package: "reylo", extensions: [".sql"] }], include: ["main.ts"] }));
  mkdirSync(join(root, "queries"));
  writeFileSync(join(root, "queries", "posts.sql"), "\n-- every post after one\nselect id, title, body\nfrom posts\nwhere id > :after\n");
  // SQLite's `@name` at the start of a line, which outside a code block TypeScript reads as a tag.
  writeFileSync(join(root, "queries", "add_post.sql"), "insert into posts (title, body)\nvalues (\n  @title,\n  @body\n)\n");
  writeFileSync(join(root, "main.ts"), `import posts from "@/queries/posts.sql";\nimport type { Row } from "@/queries/posts.sql";\nexport const query = posts;\nexport type Title = Row["title"];\nexport const sent = posts.sql;\nimport addPost from "@/queries/add_post.sql";\nexport const add = addPost;\n`);
  return root;
}

test("go-to-definition on a query, its import's path, its Row and a column opens the .sql file at its statement, and a hover shows its comment and statement", async () => {
  const root = project();
  const main = join(root, "main.ts");
  const text = readFileSync(main, "utf8");
  const at = (needle: string, offset: number): Position => {
    const before = text.slice(0, text.indexOf(needle) + offset).split("\n");
    return { line: before.length - 1, character: before.at(-1)!.length };
  };
  const lsp = languageServer(root);
  try {
    await lsp.request("initialize", { processId: process.pid, rootUri: pathToFileURL(root).href, capabilities: { textDocument: { definition: { linkSupport: true }, hover: { contentFormat: ["markdown"] } } }, initializationOptions: { runExternalCode: true } });
    lsp.notify("initialized", {});
    const uri = pathToFileURL(main).href;
    lsp.notify("textDocument/didOpen", { textDocument: { uri, languageId: "typescript", version: 1, text } });
    // The mapper analyzed the statement: the query's hover carries the row's columns.
    let hover = "";
    for (let i = 0; i < 100 && !hover.includes("title: string"); i++) {
      if (i) await new Promise((resolve) => setTimeout(resolve, 200));
      hover = (await lsp.request("textDocument/hover", { textDocument: { uri }, position: at("query = posts", 0) }))?.contents?.value ?? "";
    }
    assert.match(hover, /title: string;\s+body: string \| null;/);

    // The hover over the import shows the comment at the top of the file as prose, then the
    // statement as written; the one over its `.sql` the compiled statement, once, as its type,
    // without the comment naming the file. The doc comment's lines in the module move nothing the
    // definitions below land on.
    const hoverAt = async (position: Position): Promise<string> => (await lsp.request("textDocument/hover", { textDocument: { uri }, position }))?.contents?.value ?? "";
    const statementBlock = "```sql\nselect id, title, body\nfrom posts\nwhere id > :after\n```";
    const imported = await hoverAt(at("posts from", 0));
    assert.ok(imported.endsWith(`\nevery post after one\n\n${statementBlock}`), imported);
    const sent = await hoverAt(at("posts.sql;", 7));
    assert.match(sent, /^\(property\) sql: "\\n-- every post after one\\nselect id, title, body\\nfrom posts\\nwhere id > \?"$/m);
    assert.ok(!sent.includes("```sql"), sent);
    // A line of the statement that starts with `@` stays in the code block, not a tag after it.
    let added = "";
    for (let i = 0; i < 100 && !added.includes("```sql"); i++) {
      if (i) await new Promise((resolve) => setTimeout(resolve, 200));
      added = await hoverAt(at("addPost from", 0));
    }
    assert.ok(added.endsWith("\n```sql\ninsert into posts (title, body)\nvalues (\n  @title,\n  @body\n)\n```"), added);

    // A file URI read back as its path: on Windows the server writes the drive in lowercase with its
    // colon encoded (`file:///c%3A/…`), and the system matches a path's letters without regard to case.
    const path = (uri: string) => (process.platform === "win32" ? fileURLToPath(uri).toLowerCase() : fileURLToPath(uri));
    const sql = path(pathToFileURL(join(root, "queries", "posts.sql")).href);
    // Lines 2 to 5 of the file: the comment above the statement, through `:after`.
    const statement: Range = { start: { line: 1, character: 0 }, end: { line: 4, character: 17 } };
    for (const [what, position] of [
      ["the import's path", at(`"@/queries/posts.sql"`, 3)],
      ["the query", at("= posts", 3)],
      ["Row", at(`Row["title"]`, 1)],
      ["a column", at(`"title"]`, 2)],
    ] as const) {
      const links: { targetUri: string; targetSelectionRange: Range }[] = await lsp.request("textDocument/definition", { textDocument: { uri }, position });
      assert.deepEqual(links.map((l) => [path(l.targetUri), l.targetSelectionRange]), [[sql, statement]], what);
    }
  } finally {
    await lsp.close();
  }
});
