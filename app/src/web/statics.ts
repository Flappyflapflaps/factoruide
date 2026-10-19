// The static files of the site the pages read directly: the maps' images and the minimap's
// tiles, written by scripts/build-web.sh under data/.
import { getBackend } from "./host";

/** The game version the app works on. */
export const currentEdition = () => getBackend().call<{ edition: string }>("get_settings").then((s) => s.edition);

async function bytes(url: string): Promise<ArrayBuffer> {
  const r = await fetch(url);
  if (!r.ok) throw new Error(`${url}: ${r.status}`);
  return r.arrayBuffer();
}

export const mapImage = (edition: string, id: number) => bytes(`data/maps/${edition}/${id}.webp`);

/** `[map, level, x, y]` of each tile of an edition's minimap (none when it has no minimap). */
export async function minimapIndex(edition: string): Promise<[number, number, number, number][]> {
  const r = await fetch(`data/minimap/${edition}/index.json`);
  return r.ok ? r.json() : [];
}

export const minimapImage = (edition: string, map: number, level: number, x: number, y: number) =>
  bytes(`data/minimap/${edition}/${map}/${level}/${x}_${y}.webp`);
