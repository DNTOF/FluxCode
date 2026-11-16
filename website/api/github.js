// GET /api/github: the repository's live numbers for the website, fetched once on the
// server and cached by Vercel's CDN for every visitor.
//
// Without this every visitor called api.github.com from their own browser, 4 requests
// every refresh, and GitHub allows 60 unauthenticated requests an hour per network: a
// few open tabs (or a shared office network) used it up. Here the CDN keeps one answer
// for 5 minutes and serves it stale while it fetches the next, so GitHub sees about a
// dozen calls an hour in total. A GITHUB_TOKEN environment variable, when the project
// has one, raises GitHub's limit further; the site works without it.

const REPO = "MuaraAI/FluxCode";
const API = "https://api.github.com/repos/" + REPO;

// The last good answer of this function instance, sent again if GitHub fails.
let last = null;

async function get(path) {
  const headers = { Accept: "application/vnd.github+json", "User-Agent": "fluxcode-website" };
  if (process.env.GITHUB_TOKEN) headers.Authorization = "Bearer " + process.env.GITHUB_TOKEN;
  const res = await fetch(API + path, { headers });
  if (!res.ok) throw new Error(path + ": HTTP " + res.status);
  return { data: await res.json(), link: res.headers.get("link") || "" };
}

async function releases() {
  let all = [];
  for (let n = 1; n <= 5; n++) {
    const r = await get("/releases?per_page=100&page=" + n);
    all = all.concat(r.data);
    if (!/rel="next"/.test(r.link)) break;
  }
  // Only what the page shows: tags, dates and the installer assets' counts.
  return all.map((rel) => ({
    tag_name: rel.tag_name,
    draft: rel.draft,
    published_at: rel.published_at,
    created_at: rel.created_at,
    assets: (rel.assets || []).map((a) => ({ name: a.name, browser_download_url: a.browser_download_url, size: a.size, download_count: a.download_count })),
  }));
}

module.exports = async function handler(req, res) {
  try {
    const [repo, rels, commits, people] = await Promise.all([
      get(""),
      releases(),
      get("/commits?per_page=6"),
      get("/contributors?per_page=100"),
    ]);
    last = {
      fetchedAt: Date.now(),
      repo: { stargazers_count: repo.data.stargazers_count, forks_count: repo.data.forks_count, pushed_at: repo.data.pushed_at },
      releases: rels,
      commits: commits.data.map((c) => ({
        sha: c.sha,
        html_url: c.html_url,
        commit: { message: c.commit.message, author: c.commit.author, committer: c.commit.committer },
        author: c.author ? { login: c.author.login, avatar_url: c.author.avatar_url } : null,
      })),
      contributors: (Array.isArray(people.data) ? people.data : []).map((p) => ({ login: p.login, html_url: p.html_url, avatar_url: p.avatar_url, contributions: p.contributions })),
    };
    res.setHeader("Cache-Control", "public, s-maxage=300, stale-while-revalidate=86400");
    res.status(200).json(last);
  } catch (e) {
    if (last) {
      // GitHub failed: the previous answer is better than none, but don't cache it long.
      res.setHeader("Cache-Control", "public, s-maxage=60");
      res.status(200).json(Object.assign({ stale: true }, last));
    } else {
      res.setHeader("Cache-Control", "no-store");
      res.status(502).json({ error: String(e.message || e) });
    }
  }
};
