/**
 * The pages whose body is files of content/sections/, by the path of their file in content/docs, in
 * the order each shows them. Every section still belongs to one namespace (lib/page-files.ts), so an
 * import resolves to a statement another page shows. Here, apart from lib/page-files.ts, since the
 * browser reads it too (lib/content.ts) and that module reads the disk.
 */
export const PAGE_SECTIONS: Record<string, readonly string[]> = {
  'index.mdx': ['01-setup.mdx'],
  'usage.mdx': ['02-usage.mdx'],
  'inference.mdx': ['03-inference.mdx'],
  'configuration.mdx': ['04-configuration.mdx'],
  'observability.mdx': ['08b-tracing.mdx'],
  'faq.mdx': ['09-faq.mdx'],
};
