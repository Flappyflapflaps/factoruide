import { useCallback, useEffect, useState } from "react";
import { api, SETTINGS_CHANGED, type AddonHealth } from "../api";

/** What the game did with the addon, looked at again on these and when the window comes back (a game played meanwhile). */
export function useAddonHealth() {
  const [health, setHealth] = useState<AddonHealth | null>(null);
  const check = useCallback(() => {
    api
      .addonHealth()
      .then(setHealth)
      .catch(() => setHealth(null));
  }, []);
  useEffect(() => {
    check();
    window.addEventListener(SETTINGS_CHANGED, check);
    window.addEventListener("focus", check);
    return () => {
      window.removeEventListener(SETTINGS_CHANGED, check);
      window.removeEventListener("focus", check);
    };
  }, [check]);
  return health;
}
