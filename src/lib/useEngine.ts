// Shared window state: engine status, settings with debounced persistence,
// and the poke signal that drives the idle counter's flash.

import { useCallback, useEffect, useRef, useState } from "react";
import i18n, { resolveLanguage } from "../i18n";
import { ipc, type Settings, type StatusSnapshot } from "./ipc";
import { applyAccent, applyTheme } from "./theme";

export function useEngine() {
  const [status, setStatus] = useState<StatusSnapshot | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  /** Timestamp of the last successful poke — key for the flash animation. */
  const [pokeSignal, setPokeSignal] = useState(0);
  const saveTimer = useRef<number | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    ipc.getStatus().then(setStatus);
    ipc.getSettings().then((s) => {
      setSettings(s);
      applyTheme(s.theme);
      applyAccent(s.accent);
      i18n.changeLanguage(resolveLanguage(s.language));
    });
    ipc
      .onEngineEvent((e) => {
        if (e.type === "state_changed") {
          setStatus(e.status);
        } else {
          if (e.report.ok) setPokeSignal(Date.now());
          ipc.getStatus().then(setStatus);
        }
      })
      .then((fn) => {
        unlisten = fn;
      });
    return () => unlisten?.();
  }, []);

  const save = useCallback((next: Settings) => {
    setSettings(next);
    applyTheme(next.theme);
    applyAccent(next.accent);
    i18n.changeLanguage(resolveLanguage(next.language));
    if (saveTimer.current !== null) window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(() => {
      ipc
        .updateSettings(next)
        .then((sanitized) => {
          setSettings(sanitized);
          setSaveError(null);
        })
        .catch((e) => setSaveError(String(e)));
    }, 400);
  }, []);

  return { status, settings, save, saveError, pokeSignal };
}
