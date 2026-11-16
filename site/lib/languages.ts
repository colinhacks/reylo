/**
 * The languages an inference card shows its types in, in their order: every language choice on the page is one.
 * The type descriptor, Reylo Type Defs, is an internal representation for now, so no card or table offers it.
 */
export const LANGUAGES = ['TypeScript', 'Rust'] as const;
export type LanguageName = (typeof LANGUAGES)[number];
