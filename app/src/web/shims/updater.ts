// `@tauri-apps/plugin-updater` in the web build: the site is always the latest version.
export type Update = { version: string; body?: string; downloadAndInstall(cb?: unknown): Promise<void> };
export async function check(): Promise<Update | null> {
  return null;
}
