// Activity section: the weekly report as big stat tiles + espresso shots.

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { formatSeconds } from "../../lib/format";
import { ipc, type StatsReport } from "../../lib/ipc";

function Tile({
  label,
  value,
  accent = false,
}: {
  label: string;
  value: string;
  accent?: boolean;
}) {
  return (
    <div className="card flex flex-col gap-1 p-5">
      <span className="text-xs uppercase tracking-widest text-ink-2">
        {label}
      </span>
      <span
        className={`display text-3xl ${accent ? "text-accent" : "text-ink"}`}
      >
        {value}
      </span>
    </div>
  );
}

export function ActivityPanel() {
  const { t } = useTranslation();
  const [report, setReport] = useState<StatsReport | null>(null);

  useEffect(() => {
    ipc.getStats().then(setReport, () => setReport(null));
  }, []);

  if (!report) return null;

  return (
    <div className="space-y-5">
      <div className="grid grid-cols-2 gap-5 lg:grid-cols-4">
        <Tile
          label={t("report.activeTime")}
          value={formatSeconds(report.week_active_secs)}
          accent
        />
        <Tile label={t("report.pokes")} value={String(report.week_pokes)} />
        <Tile label={t("report.shots")} value={`☕ ×${report.shots_today}`} />
        {report.health_now != null && report.health_month_ago != null ? (
          <Tile
            label={t("report.healthTrend")}
            value={`${report.health_month_ago.toFixed(1)} → ${report.health_now.toFixed(1)}%`}
          />
        ) : (
          <Tile label={t("report.healthTrend")} value="—" />
        )}
      </div>
    </div>
  );
}
