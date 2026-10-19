// `@tauri-apps/plugin-dialog` in the web build: no folder of the computer to pick. Asked for a
// folder to export to, the answer is "" (the export is downloaded instead); the pages choosing
// the game's folder are not shown in the browser.
export async function open(_options?: unknown): Promise<string | string[] | null> {
  return "";
}
