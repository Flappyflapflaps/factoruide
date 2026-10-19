// `@tauri-apps/plugin-dialog` in the web build: no folder of the computer to pick (the pages
// using it are not shown in the browser).
export async function open(_options?: unknown): Promise<string | string[] | null> {
  return null;
}
