import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, realpathSync, symlinkSync, writeFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

// Go-to-definition from what a `.sql` module declares opens the `.sql` file at its statement, through
// the span map the content mapper returns (src/mapper.ts), and a hover shows the file's comment and
// statement, from the doc comments the module carries (src/doc-comment.ts). The language server
// runs as an editor runs it, `tsc --lsp --stdio` with runExternalCode on, over a SQLite project, so
// no server is needed.
const ROOT = join(import.meta.dirname, "..");
const TSC = join(ROOT, "node_modules", ".bin", "tsc");

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

/** A language server over stdio with LSP framing, as an editor talks to it. */
function server(root: string) {
  const child = spawn(TSC, ["--lsp", "--stdio"], { cwd: root, stdio: ["pipe", "pipe", "ignore"] });
  const pending = new Map<number, (result: any) => void>();
  let buffer = Buffer.alloc(0);
  const write = (message: object) => {
    const body = Buffer.from(JSON.stringify({ jsonrpc: "2.0", ...message }), "utf8");
    child.stdin.write(`Content-Length: ${body.length}\r\n\r\n`);
    child.stdin.write(body);
  };
  child.stdout.on("data", (chunk: Buffer) => {
    buffer = Buffer.concat([buffer, chunk]);
    for (;;) {
      const end = buffer.indexOf("\r\n\r\n");
      if (end === -1) return;
      const length = Number(/Content-Length:\s*(\d+)/i.exec(buffer.subarray(0, end).toString("ascii"))![1]);
      if (buffer.length < end + 4 + length) return;
      const message = JSON.parse(buffer.subarray(end + 4, end + 4 + length).toString("utf8"));
      buffer = buffer.subarray(end + 4 + length);
      // A request from the server (registerCapability, workspace/configuration) is answered as a
      // client that has nothing to add.
      if (message.method && message.id !== undefined) write({ id: message.id, result: message.method === "workspace/configuration" ? message.params.items.map(() => null) : null });
      else if (message.id !== undefined) pending.get(message.id)?.(message.result);
    }
  });
  let id = 0;
  const request = (method: string, params: unknown) =>
    new Promise<any>((resolve) => {
      pending.set(++id, resolve);
      write({ id, method, params });
    });
  const notify = (method: string, params: unknown) => write({ method, params });
  const close = async () => {
    await request("shutdown", undefined);
    notify("exit", undefined);
    await new Promise((resolve) => child.once("exit", resolve));
  };
  return { request, notify, close };
}

test("go-to-definition on a query, its import's path, its Row and a column opens the .sql file at its statement, and a hover shows its comment and statement", async () => {
  const root = project();
  const main = join(root, "main.ts");
  const text = readFileSync(main, "utf8");
  const at = (needle: string, offset: number): Position => {
    const before = text.slice(0, text.indexOf(needle) + offset).split("\n");
    return { line: before.length - 1, character: before.at(-1)!.length };
  };
  const lsp = server(root);
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
    // statement as written; the one over its `.sql` the statement as Reylo sends it, the comment
    // taken off both. The doc comments' lines in the module move nothing the definitions below
    // land on.
    const hoverAt = async (position: Position): Promise<string> => (await lsp.request("textDocument/hover", { textDocument: { uri }, position }))?.contents?.value ?? "";
    const statementBlock = "```sql\nselect id, title, body\nfrom posts\nwhere id > :after\n```";
    const imported = await hoverAt(at("posts from", 0));
    assert.ok(imported.endsWith(`\nevery post after one\n\n${statementBlock}`), imported);
    const sent = await hoverAt(at("posts.sql;", 7));
    assert.match(sent, /^\(property\) sql: "\\n-- every post after one\\nselect id, title, body\\nfrom posts\\nwhere id > \? \/\*file='queries%2Fposts\.sql'\*\/"$/m);
    assert.ok(sent.includes("\nevery post after one\n\n```sql\nselect id, title, body\nfrom posts\nwhere id > ?\n```\n\nThe statement as Reylo sends it"), sent);
    // A line of the statement that starts with `@` stays in the code block, not a tag after it.
    let added = "";
    for (let i = 0; i < 100 && !added.includes("```sql"); i++) {
      if (i) await new Promise((resolve) => setTimeout(resolve, 200));
      added = await hoverAt(at("addPost from", 0));
    }
    assert.ok(added.endsWith("\n```sql\ninsert into posts (title, body)\nvalues (\n  @title,\n  @body\n)\n```"), added);

    const sql = pathToFileURL(join(root, "queries", "posts.sql")).href;
    // Lines 2 to 5 of the file: the comment above the statement, through `:after`.
    const statement: Range = { start: { line: 1, character: 0 }, end: { line: 4, character: 17 } };
    for (const [what, position] of [
      ["the import's path", at(`"@/queries/posts.sql"`, 3)],
      ["the query", at("= posts", 3)],
      ["Row", at(`Row["title"]`, 1)],
      ["a column", at(`"title"]`, 2)],
    ] as const) {
      const links: { targetUri: string; targetSelectionRange: Range }[] = await lsp.request("textDocument/definition", { textDocument: { uri }, position });
      assert.deepEqual(links.map((l) => [l.targetUri, l.targetSelectionRange]), [[sql, statement]], what);
    }
  } finally {
    await lsp.close();
  }
});
