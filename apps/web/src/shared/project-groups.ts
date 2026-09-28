export interface ProjectIdentity {
  sourceId: string;
  sourceName: string;
  repositoryId: string;
  repositoryOwner: string;
  repositoryName: string;
}

export interface ProjectItems<T> extends ProjectIdentity {
  id: string;
  items: T[];
}

// sourceId identifies a configured connection, not just the provider type.
export function groupByProject<T extends ProjectIdentity>(items: T[]): ProjectItems<T>[] {
  const projects = new Map<string, ProjectItems<T>>();
  for (const item of items) {
    const id = JSON.stringify([item.sourceId, item.repositoryId]);
    const existing = projects.get(id);
    if (existing) {
      existing.items.push(item);
    } else {
      projects.set(id, {
        id,
        sourceId: item.sourceId,
        sourceName: item.sourceName,
        repositoryId: item.repositoryId,
        repositoryOwner: item.repositoryOwner,
        repositoryName: item.repositoryName,
        items: [item],
      });
    }
  }
  return [...projects.values()].sort(
    (left, right) =>
      `${left.repositoryOwner}/${left.repositoryName}`.localeCompare(
        `${right.repositoryOwner}/${right.repositoryName}`,
        undefined,
        { sensitivity: "base" },
      ) || left.sourceId.localeCompare(right.sourceId),
  );
}
