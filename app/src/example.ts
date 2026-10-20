import { api, type Edition } from "./api";
import type { OpenGuide } from "./pages/GuidesPage";

/**
 * The example guide shipped with the apps (WoW Forever, human paladin 1 to 30, made by
 * scripts/example-guide.sh): its id among the guides, made from its name.
 */
export const EXAMPLE_GUIDE = "exemple-humain-paladin-1-30";

/** The example is a WoW Forever guide: offered on that game version only. */
export const hasExample = (edition: Edition | null) => edition === "forever";

/**
 * The example among the guides, added the first time (the .fgguide import, as a file received):
 * the version to open, the one its page says to follow when it is already there.
 */
export async function exampleGuide(): Promise<OpenGuide> {
  const found = (await api.listGuides()).find((g) => g.id === EXAMPLE_GUIDE);
  if (found?.versions.length) {
    const shown = found.versions.find((v) => v.version === found.installed) ?? found.versions[0];
    return { guide: found.id, version: shown.version, notice: null };
  }
  // Loaded only when asked (a file of its own in the build).
  const file = (await import("./assets/example-guide.fgguide?raw")).default;
  const meta = await api.importGuide(file);
  return { guide: meta.guide, version: meta.version, notice: null };
}
