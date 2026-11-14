// The diagnostics Reylo's content mapper gives a `.sql` file, asked of the mapper itself: the capture
// (scripts/capture-types.ts) starts `src/mapper.ts` as TypeScript starts it, a process speaking
// JSON-RPC over stdio, opens a project in a scratch directory whose `DATABASE_URL` is a scratch
// database, and sends each error example of the page as a file to transform. What comes back is what
// the editor and `tsc` show for that file: each error's message, code and place, the database's own
// (`reylo6`) or Reylo's (`reylo20`), never typed by hand.
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import path from 'node:path';
import { reportedDiagnostics, type Reported } from '../../src/mapper-diagnostics.ts';

/** One diagnostic as the mapper returns it: its place in the file in UTF-8 bytes, its code (`reylo<code>`) and its message. */
export type MapperDiagnostic = { start: number; length: number; code: number; messageText: string };

const MAPPER = path.resolve(import.meta.dirname, '../../src/mapper.ts');

export type Mapper = {
  /** The diagnostics of a file of the project with this text. */
  diagnostics(fileName: string, content: string): Promise<MapperDiagnostic[]>;
  close(): Promise<void>;
};

/** Starts the mapper for the project in `dir`, with `env` over the capture's own environment. */
export async function openMapper(dir: string, env: Record<string, string>): Promise<Mapper> {
  const child: ChildProcessWithoutNullStreams = spawn(process.execPath, [MAPPER], { cwd: dir, env: { ...process.env, ...env }, stdio: ['pipe', 'pipe', 'pipe'] });
  let stderr = '';
  child.stderr.on('data', (chunk: Buffer) => (stderr += chunk.toString('utf8')));
  const pending = new Map<number, { resolve(value: unknown): void; reject(error: Error): void }>();
  let buffer = Buffer.alloc(0);
  child.stdout.on('data', (chunk: Buffer) => {
    buffer = Buffer.concat([buffer, chunk]);
    for (;;) {
      const end = buffer.indexOf('\r\n\r\n');
      if (end === -1) return;
      const length = Number(/Content-Length:\s*(\d+)/i.exec(buffer.subarray(0, end).toString('ascii'))?.[1]);
      if (buffer.length < end + 4 + length) return;
      const message = JSON.parse(buffer.subarray(end + 4, end + 4 + length).toString('utf8')) as { id: number; result?: unknown; error?: { message: string } };
      buffer = buffer.subarray(end + 4 + length);
      const waiting = pending.get(message.id);
      pending.delete(message.id);
      if (message.error) waiting?.reject(new Error(message.error.message));
      else waiting?.resolve(message.result);
    }
  });
  const exited = new Promise<void>((resolve) => child.on('exit', () => resolve()));
  child.on('exit', (code) => {
    for (const { reject } of pending.values()) reject(new Error(`the content mapper exited (${code}) before it answered${stderr ? `:\n${stderr}` : ''}`));
    pending.clear();
  });
  let id = 0;
  const call = (method: string, params: unknown) =>
    new Promise<unknown>((resolve, reject) => {
      const payload = Buffer.from(JSON.stringify({ jsonrpc: '2.0', id: ++id, method, params }), 'utf8');
      pending.set(id, { resolve, reject });
      child.stdin.write(`Content-Length: ${payload.length}\r\n\r\n`);
      child.stdin.write(payload);
    });
  const projectHandle = 'capture';
  await call('initialize', { protocolVersion: 1 });
  await call('openProject', { projectHandle, configFileName: path.join(dir, 'tsconfig.json'), options: {} });
  return {
    async diagnostics(fileName, content) {
      return reportedDiagnostics((await call('transform', { fileName: path.join(dir, fileName), content, projectHandle })) as Reported);
    },
    async close() {
      await call('closeProject', { projectHandle }).catch(() => {});
      child.stdin.end();
      await exited;
    },
  };
}

/** A position in the editor's terms, from 1, in JavaScript's characters: the line and column of a UTF-8 byte offset into `text`. */
export function positionOf(text: string, byte: number): { line: number; column: number } {
  const before = Buffer.from(text, 'utf8').subarray(0, byte).toString('utf8');
  const lines = before.split('\n');
  return { line: lines.length, column: lines.at(-1)!.length + 1 };
}
