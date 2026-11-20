// Vite's types for `?url` imports (the fonts app/root.tsx preloads) and import.meta.env.
/// <reference types="vite/client" />

// The comparison's popover cards, built by lib/compare-cards.ts.
declare module 'virtual:compare-cards' {
  const cards: Record<string, import('@/lib/compare-cards').CompareCard>;
  export default cards;
}

// The homepage's examples, the docs' schemas and the intro's import, built by lib/home-examples.ts.
declare module 'virtual:home-examples' {
  const data: import('@/lib/home-examples').HomeData;
  export default data;
}
