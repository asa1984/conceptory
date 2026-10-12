// Represents the screen to show as a URL.
//
// Document IDs may contain `.` segments as in `./docs/a.md`, which browsers resolve in a URL path.
// Query strings are left as is, so the URL takes the form `/?doc=<id>`.

export type Route =
  | { view: "home" }
  | { view: "document"; id: string }
  | { view: "terms" }
  | { view: "problems" };

export const parseRoute = (url: URL): Route => {
  const id = url.searchParams.get("doc");
  if (id !== null) {
    return { view: "document", id };
  }
  switch (url.searchParams.get("view") ?? "") {
    case "terms": {
      return { view: "terms" };
    }
    case "problems": {
      return { view: "problems" };
    }
    default: {
      return { view: "home" };
    }
  }
};

/** Leaves `/` unencoded for readability, like the links the server rewrites. */
const encodeQuery = (value: string): string => encodeURIComponent(value).replaceAll("%2F", "/");

export const href = (route: Route): string => {
  switch (route.view) {
    case "home": {
      return "/";
    }
    case "document": {
      return `/?doc=${encodeQuery(route.id)}`;
    }
    case "terms":
    case "problems": {
      return `/?view=${route.view}`;
    }
  }
};

/**
 * Maps the `#x` of a link to a heading or footnote to an element in the body. Sanitization
 * prefixes body ids with `user-content-`, so this looks up the prefixed id, as GitHub does.
 */
export const scrollToHash = (hash: string): void => {
  if (hash === "") {
    return;
  }
  const name = CSS.escape(decodeURIComponent(hash.slice(1)));
  const element =
    document.querySelector(`[id="user-content-${name}"]`) ??
    document.querySelector(`[id="${name}"]`);
  element?.scrollIntoView();
};
