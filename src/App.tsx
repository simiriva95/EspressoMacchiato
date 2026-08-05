import { Hud } from "./windows/Hud";
import { Dashboard } from "./windows/Dashboard";

function getWindowLabel(): string {
  // Browser dev preview: pick the window via ?window=popover.
  try {
    const params = new URLSearchParams(window.location.search);
    const override = params.get("window");
    if (override) return override;
  } catch {
    /* no location in exotic environments */
  }
  try {
    // Lazy require so the module resolves outside Tauri too.
    const meta = (
      window as unknown as {
        __TAURI_INTERNALS__?: { metadata?: { currentWebview?: { label?: string } } };
      }
    ).__TAURI_INTERNALS__;
    return meta?.metadata?.currentWebview?.label ?? "main";
  } catch {
    return "main";
  }
}

function App() {
  const label = getWindowLabel();
  if (label === "hud") return <Hud />;
  return <Dashboard />;
}

export default App;
