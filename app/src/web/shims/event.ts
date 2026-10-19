// `@tauri-apps/api/event` in the web build: the backend's events (host.ts), and those the page
// sends itself.
import { getBackend } from "../host";

export type UnlistenFn = () => void;
export type Event<T> = { event: string; payload: T };

const local = new EventTarget();

export async function listen<T>(event: string, handler: (e: Event<T>) => void): Promise<UnlistenFn> {
  const fromBackend = getBackend().listen(event, (payload) => handler({ event, payload: payload as T }));
  const onLocal = (e: globalThis.Event) => handler({ event, payload: (e as CustomEvent<T>).detail });
  local.addEventListener(event, onLocal);
  return () => {
    fromBackend();
    local.removeEventListener(event, onLocal);
  };
}

export async function emit(event: string, payload?: unknown): Promise<void> {
  local.dispatchEvent(new CustomEvent(event, { detail: payload }));
}
