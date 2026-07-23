import { useCallback, useEffect, useRef, useState } from "react";
import { getCareerAchievements } from "../../shared/api";
import { productErrorMessage } from "../../shared/phase6Shell";
import type { CareerAchievement } from "../../shared/types";
import { createLatestRequestGuard } from "./usePersonalSeasonReview";

export function useCareerAchievements() {
  const [achievements, setAchievements] = useState<CareerAchievement[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const guard = useRef(createLatestRequestGuard());

  const requestAchievements = useCallback(() => {
    const request = guard.current.begin();
    setLoading(true);
    setError(null);
    void getCareerAchievements()
      .then((result) => {
        if (!guard.current.isCurrent(request)) return;
        setAchievements(result);
      })
      .catch((reason) => {
        if (guard.current.isCurrent(request)) setError(productErrorMessage(reason));
      })
      .finally(() => {
        if (guard.current.isCurrent(request)) setLoading(false);
      });
    return () => guard.current.invalidate(request);
  }, []);

  useEffect(() => requestAchievements(), [requestAchievements]);
  return { achievements, error, loading, requestAchievements };
}
