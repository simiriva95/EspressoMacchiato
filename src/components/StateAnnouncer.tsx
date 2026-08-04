// Screen-reader announcements, throttled by design: only state transitions
// are announced, never the ticking counter (spec §7 quality floor).

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { StatusSnapshot } from "../lib/ipc";

export function StateAnnouncer({ status }: { status: StatusSnapshot | null }) {
  const { t } = useTranslation();
  const [message, setMessage] = useState("");
  const lastState = useRef<string | null>(null);

  useEffect(() => {
    if (status && status.state !== lastState.current) {
      lastState.current = status.state;
      setMessage(
        t("a11y.stateAnnouncement", { state: t(`state.${status.state}`) }),
      );
    }
  }, [status, t]);

  return (
    <div aria-live="polite" className="visually-hidden">
      {message}
    </div>
  );
}
