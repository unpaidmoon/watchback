import { useEffect } from "react";
import { useLocation } from "react-router-dom";

export function RouteFocusManager() {
  const { pathname, hash } = useLocation();

  useEffect(() => {
    if (!hash) {
      return;
    }

    const frame = requestAnimationFrame(() => {
      const target = document.getElementById(hash.slice(1));
      if (!target) {
        return;
      }

      target.scrollIntoView({ block: "start" });
      target.focus({ preventScroll: true });
    });

    return () => cancelAnimationFrame(frame);
  }, [pathname, hash]);

  return null;
}
