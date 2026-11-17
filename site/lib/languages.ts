/**
 * Whether the docs show Rust: its page (content/docs/rust.mdx), every `<Rust>` passage and Rust
 * panel of the other pages, and the Rust types on the inference cards. While it is off, the Rust
 * content stays in the pages, and lib/rust-content.ts leaves it out of the build.
 */
export const SHOW_RUST = false;

/** Every language the docs are written for. */
export type LanguageName = 'TypeScript' | 'Rust';

/**
 * The languages an inference card shows its types in, in their order: every language choice on the page is one.
 * The type descriptor, Reylo Type Defs, is an internal representation for now, so no card or table offers it.
 * With one language, no switch is drawn.
 */
export const LANGUAGES: readonly LanguageName[] = SHOW_RUST ? ['TypeScript', 'Rust'] : ['TypeScript'];
