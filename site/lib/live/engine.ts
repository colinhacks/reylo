// The Postgres engine, instantiated from its compiled module with the playground's own WASI
// (lib/live/wasi.ts), in the worker and at build time alike.
import { EngineCore, type Instance } from '../../../src/engine-core.ts';
import { wasiImports } from './wasi.ts';

export async function instantiateEngine(module: WebAssembly.Module): Promise<EngineCore> {
  let instance!: WebAssembly.Instance;
  instance = await WebAssembly.instantiate(module, { wasi_snapshot_preview1: wasiImports(() => instance.exports.memory as WebAssembly.Memory) });
  // A reactor's constructors, where the toolchain gave it any.
  (instance.exports._initialize as (() => void) | undefined)?.();
  return new EngineCore(instance as unknown as Instance);
}
