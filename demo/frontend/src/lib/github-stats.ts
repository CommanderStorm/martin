export interface GithubStats {
  stars: string;
  contributors: string;
  forks: string;
  latestVersion: string;
}

const FALLBACK: GithubStats = {
  contributors: '100+',
  forks: '400+',
  latestVersion: 'v1.x',
  stars: '4k+',
};

function fmt(n: number): string {
  if (n >= 1000) return `${(n / 1000).toFixed(1).replace(/\.0$/, '')}k`;
  return String(n);
}

export async function getGithubStats(): Promise<GithubStats> {
  try {
    const headers: HeadersInit = {
      Accept: 'application/vnd.github+json',
      'X-GitHub-Api-Version': '2022-11-28',
    };
    if (typeof process !== 'undefined' && process.env?.GITHUB_TOKEN) {
      headers.Authorization = `Bearer ${process.env.GITHUB_TOKEN}`;
    }

    const [repoRes, contribRes, releaseRes] = await Promise.all([
      fetch('https://api.github.com/repos/maplibre/martin', { headers }),
      fetch('https://api.github.com/repos/maplibre/martin/contributors?per_page=1&anon=true', {
        headers,
      }),
      fetch('https://api.github.com/repos/maplibre/martin/releases/latest', { headers }),
    ]);

    if (!repoRes.ok) return FALLBACK;

    const repo = (await repoRes.json()) as { stargazers_count: number; forks_count: number };
    const link = contribRes.headers.get('link') ?? '';
    const match = link.match(/page=(\d+)>; rel="last"/);
    const contributors = match ? parseInt(match[1], 10) : null;
    const release = releaseRes.ok
      ? ((await releaseRes.json()) as { tag_name: string | null })
      : null;

    return {
      contributors: contributors != null ? `${fmt(contributors)}+` : '100+',
      forks: fmt(repo.forks_count),
      latestVersion: release?.tag_name ?? 'v0.x',
      stars: fmt(repo.stargazers_count),
    };
  } catch {
    return FALLBACK;
  }
}
