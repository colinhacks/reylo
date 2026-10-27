// The loader's sources the site imports (lib/playground.ts, through ../src/analyze.ts) reach the
// engine's host, src/component.ts, which imports jco's preview2-shim from the repository root's
// install. The site is built with its own install alone, so there the root's module is not found and
// this declaration stands in for it; with the root installed, TypeScript reads the real one.
declare module '@bytecodealliance/preview2-shim/instantiation';
