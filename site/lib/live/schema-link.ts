// A schema of the reader's own in /play's address, `?schema=<text>`: its DDL compressed with deflate-raw and written in base64url, so that a few kilobytes of DDL stay a link that can be pasted anywhere. The browser's CompressionStream does both directions, so nothing is bundled for it.

async function through(bytes: Uint8Array, stream: CompressionStream | DecompressionStream): Promise<Uint8Array> {
  return new Uint8Array(await new Response(new Blob([bytes as BlobPart]).stream().pipeThrough(stream)).arrayBuffer());
}

/** The DDL as the address carries it. */
export async function encodeSchema(ddl: string): Promise<string> {
  const bytes = await through(new TextEncoder().encode(ddl), new CompressionStream('deflate-raw'));
  let binary = '';
  for (const b of bytes) binary += String.fromCharCode(b);
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

/** The DDL from the address's `schema`; it throws on a value that is not one this page wrote. */
export async function decodeSchema(value: string): Promise<string> {
  const binary = atob(value.replace(/-/g, '+').replace(/_/g, '/'));
  const bytes = Uint8Array.from(binary, (c) => c.charCodeAt(0));
  return new TextDecoder('utf-8', { fatal: true }).decode(await through(bytes, new DecompressionStream('deflate-raw')));
}
