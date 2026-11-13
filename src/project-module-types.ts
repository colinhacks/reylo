// The declarations of the project file's module, which the content mapper gives an import of
// `.reylo.yaml` (src/mapper-server.ts, and src/mapper.ts where there is no engine): what the module
// the bundler plugin writes for it holds (src/project-module.ts).

export const PROJECT_MODULE_TYPES = `/**
 * Runs this project's migrations on a SQLite-backed Durable Object's storage: each it hasn't run yet,
 * in the order \`reylo migrate deploy\` runs them, each in a transaction with its row in
 * \`__reylo_migrations\`. Returns the names it ran. Call it in the object's constructor:
 * \`migrate(ctx.storage)\`. Reylo's bundler plugin puts the migrations in the bundle.
 */
export declare function migrate(storage: { readonly sql: { exec(query: string, ...bindings: any[]): { toArray(): unknown[] } }; transactionSync<T>(closure: () => T): T }): string[];
`;
