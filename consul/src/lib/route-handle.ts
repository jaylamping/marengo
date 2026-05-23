export type RouteHeaderConfig = {
  title: string;
  subtitle?: string;
};

export type RouteHandle = {
  header?: RouteHeaderConfig;
};

export function getRouteHeader(matches: Array<{ handle?: unknown }>): RouteHeaderConfig | undefined {
  for (let index = matches.length - 1; index >= 0; index -= 1) {
    const handle = matches[index]?.handle as RouteHandle | undefined;
    if (handle?.header) {
      return handle.header;
    }
  }

  return undefined;
}
