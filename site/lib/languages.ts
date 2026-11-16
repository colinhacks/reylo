/** The languages an inference card shows its types in, in their order: every language choice on the page is one. */
export const LANGUAGES = ['TypeScript', 'Rust', 'Type Defs'] as const;
export type LanguageName = (typeof LANGUAGES)[number];
