/** The star count once read, for an hour: GitHub answers 60 unauthenticated requests an hour, which a dev server's reloads use up. */
let cached: { stars: number; at: number } | undefined;

/** Reylo's star count on GitHub, for a star pill, or none when GitHub can't be reached. Read where a page is rendered, at build time. */
export async function fetchStars(): Promise<number | undefined> {
  if (cached && Date.now() - cached.at < 3_600_000) return cached.stars;
  try {
    const res = await fetch('https://api.github.com/repos/reylosql/reylo', { headers: { Accept: 'application/vnd.github+json' } });
    const repo = (await res.json()) as { stargazers_count?: number };
    if (!res.ok || repo.stargazers_count === undefined) return undefined;
    cached = { stars: repo.stargazers_count, at: Date.now() };
    return repo.stargazers_count;
  } catch {
    return undefined;
  }
}
