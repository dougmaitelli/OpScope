import "./CommitLink.css";

// Resource URLs retain the provider's host and any installation/namespace prefix.
// Match repository routes rather than provider names, which can be custom labels.
function commitUrl(resourceUrl: string, sha: string): string | null {
  if (!sha) return null;
  try {
    const url = new URL(resourceUrl);
    if (url.protocol !== "https:" && url.protocol !== "http:") return null;
    const routes: Array<[RegExp, string]> = [
      [/^(.*)\/-\/(?:pipelines|merge_requests)\/[^/]+(?:\/.*)?$/, "/-/commit/"],
      [/^(.*)\/(?:pipelines\/results|pull-requests)\/[^/]+(?:\/.*)?$/, "/commits/"],
      [/^(.*)\/addon\/pipelines\/home\/?$/, "/commits/"],
      [/^(.*)\/(?:actions\/runs|pull|pulls)\/[^/]+(?:\/.*)?$/, "/commit/"],
    ];
    for (const [route, commitPath] of routes) {
      const repositoryPath = url.pathname.match(route)?.[1];
      if (repositoryPath) {
        url.pathname = `${repositoryPath}${commitPath}${encodeURIComponent(sha)}`;
        url.search = "";
        url.hash = "";
        return url.href;
      }
    }
  } catch {
    // Keep the SHA readable when the provider returned an incomplete resource URL.
  }
  return null;
}

export function CommitLink({ sha, resourceUrl }: { sha: string; resourceUrl: string }) {
  const href = commitUrl(resourceUrl, sha);
  const text = sha.slice(0, 7);
  return href ? (
    <a
      className="commit-link"
      href={href}
      title={sha}
      aria-label={`Open commit ${sha}`}
      target="_blank"
      rel="noreferrer"
      onClick={(event) => event.stopPropagation()}
      onKeyDown={(event) => event.stopPropagation()}
    >
      {text}
    </a>
  ) : (
    <span title={sha}>{text}</span>
  );
}
