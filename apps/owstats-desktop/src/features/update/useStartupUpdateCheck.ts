import { useCallback, useEffect, useState } from "react";
import { checkAppUpdate } from "../../shared/api";
import { hasDesktopBackend } from "../../shared/phase6Shell";
import type { UpdateAnnouncement, UpdateCheckResult } from "../../shared/types";

export function announcementFromUpdateResult(result: UpdateCheckResult): UpdateAnnouncement | null {
  return result.status === "updateAvailable" ? result.announcement : null;
}

export type StartupUpdateState = {
  announcement: UpdateAnnouncement | null;
  currentVersion: string | null;
  dismiss(): void;
};

export function useStartupUpdateCheck(): StartupUpdateState {
  const [announcement, setAnnouncement] = useState<UpdateAnnouncement | null>(null);
  const [currentVersion, setCurrentVersion] = useState<string | null>(null);

  useEffect(() => {
    if (!hasDesktopBackend()) return;
    let disposed = false;
    void checkAppUpdate()
      .then((result) => {
        if (disposed) return;
        setCurrentVersion(result.currentVersion);
        setAnnouncement(announcementFromUpdateResult(result));
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
    };
  }, []);

  const dismiss = useCallback(() => setAnnouncement(null), []);
  return { announcement, currentVersion, dismiss };
}
