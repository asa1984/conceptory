// Computes data versions from server notifications.
//
// Versions are part of cache keys; bumping one makes screens refetch with a new Promise.

import type { Hello, Update } from "./data.ts";

export type Versions = {
  serverId: string | null;
  revision: number;
  /** Incremented when the server restarts so that all data is refetched. */
  epoch: number;
  /** Revision at which the overview last changed. */
  overview: number;
  /** Revision at which each document last changed. */
  documents: ReadonlyMap<string, number>;
};

export const initialVersions: Versions = {
  serverId: null,
  revision: 0,
  epoch: 0,
  overview: 0,
  documents: new Map(),
};

export const applyHello = ({ versions, hello }: { versions: Versions; hello: Hello }): Versions => {
  // On the first connection, the initially fetched data is still valid.
  if (versions.serverId === null) {
    return { ...versions, serverId: hello.serverId, revision: hello.revision };
  }
  // Refetch everything if the server restarted or something changed while disconnected.
  if (hello.serverId !== versions.serverId || hello.revision !== versions.revision) {
    return {
      ...versions,
      serverId: hello.serverId,
      revision: hello.revision,
      epoch: versions.epoch + 1,
    };
  }
  return versions;
};

export const applyUpdate = ({
  versions,
  event,
}: {
  versions: Versions;
  event: Update & { type: "changed" };
}): Versions => {
  // Missed notifications leave no changes to apply incrementally, so refetch everything.
  if (event.revision !== versions.revision + 1) {
    return { ...versions, revision: event.revision, epoch: versions.epoch + 1 };
  }
  const documents = new Map(versions.documents);
  for (const id of [...event.changed, ...event.removed]) {
    documents.set(id, event.revision);
  }
  return {
    ...versions,
    revision: event.revision,
    overview: event.overview ? event.revision : versions.overview,
    documents,
  };
};

export const resync = (versions: Versions): Versions => ({
  ...versions,
  epoch: versions.epoch + 1,
});

export const overviewVersion = (versions: Versions): string =>
  `${versions.epoch.toString()}:${versions.overview.toString()}`;

export const documentVersion = ({ versions, id }: { versions: Versions; id: string }): string =>
  `${versions.epoch.toString()}:${(versions.documents.get(id) ?? 0).toString()}`;
