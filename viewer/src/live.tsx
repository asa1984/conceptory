// Receives change notifications from the server and bumps data versions in a transition.
//
// Bumping a version makes screens that read documents or the overview suspend on a new Promise.
// Because the bump happens in a transition, the previous content stays visible until the fetch
// finishes and is then replaced all at once.

import { startTransition, useEffect, useEffectEvent, useState } from "react";
import type { ReactNode } from "react";

import { decodeJson } from "./data.ts";
import type { Hello, Update } from "./data.ts";
import { LiveContext } from "./live-context.ts";
import type { Connection } from "./live-context.ts";
import {
  applyHello,
  applyUpdate,
  documentVersion,
  initialVersions,
  overviewVersion,
  resync,
} from "./live-versions.ts";

type Handlers = {
  onHello: (hello: Hello) => void;
  onUpdate: (event: Update) => void;
  onResync: () => void;
  onConnection: (connection: Connection) => void;
};

/**
 * Subscribes to server notifications. Browsers allow at most six HTTP/1.1 connections per host,
 * so many open tabs would run out of connections. Hidden tabs disconnect and reconnect when shown.
 */
const subscribeEvents = (handlers: Handlers): (() => void) => {
  let source: EventSource | null = null;
  let retry: ReturnType<typeof setTimeout> | undefined;

  const disconnect = (): void => {
    clearTimeout(retry);
    source?.close();
    source = null;
  };
  const connect = (): void => {
    if (source !== null) {
      return;
    }
    handlers.onConnection("connecting");
    const current = new EventSource("/api/events");
    current.addEventListener("hello", (message) => {
      handlers.onHello(decodeJson<Hello>(String(message.data)));
    });
    current.addEventListener("update", (message) => {
      handlers.onUpdate(decodeJson<Update>(String(message.data)));
    });
    current.addEventListener("resync", handlers.onResync);
    current.addEventListener("error", () => {
      // EventSource reconnects automatically when the connection drops, but not after an error response.
      if (current.readyState === EventSource.CLOSED) {
        disconnect();
        handlers.onConnection("closed");
        retry = setTimeout(connect, 2000);
      } else {
        handlers.onConnection("connecting");
      }
    });
    source = current;
  };
  const onVisibilityChange = (): void => {
    if (document.visibilityState === "visible") {
      connect();
    } else {
      disconnect();
    }
  };

  document.addEventListener("visibilitychange", onVisibilityChange);
  connect();
  return () => {
    document.removeEventListener("visibilitychange", onVisibilityChange);
    disconnect();
  };
};

export const LiveProvider = ({ children }: { children: ReactNode }) => {
  const [versions, setVersions] = useState(initialVersions);
  const [connection, setConnection] = useState<Connection>("connecting");
  const [reloadError, setReloadError] = useState<string | null>(null);

  const onHello = useEffectEvent((hello: Hello) => {
    setConnection("open");
    startTransition(() => {
      setVersions((current) => applyHello({ versions: current, hello }));
    });
  });
  const onUpdate = useEffectEvent((event: Update) => {
    if (event.type === "error") {
      setReloadError(event.message);
      return;
    }
    startTransition(() => {
      setReloadError(null);
      setVersions((current) => applyUpdate({ versions: current, event }));
    });
  });
  const onResync = useEffectEvent(() => {
    startTransition(() => {
      setVersions(resync);
    });
  });
  const onConnection = useEffectEvent((next: Connection) => {
    setConnection(next);
  });

  // The dependency array is an optimization to avoid reconnecting on every render.
  useEffect(() => subscribeEvents({ onHello, onUpdate, onResync, onConnection }), []);

  const live = {
    connection,
    reloadError,
    overviewVersion: overviewVersion(versions),
    documentVersion: (id: string) => documentVersion({ versions, id }),
  };
  return <LiveContext value={live}>{children}</LiveContext>;
};
