import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  type ReactNode,
} from "react";

type UnsavedChangesRegistration = {
  isDirty: boolean;
  confirmDiscard: () => boolean;
};

type NavigationEvent = {
  preventDefault: () => void;
};

type UnsavedChangesGuardValue = {
  register: (guard: UnsavedChangesRegistration | null) => void;
  guardNavigation: (event?: NavigationEvent) => boolean;
};

const UnsavedChangesGuardContext =
  createContext<UnsavedChangesGuardValue | null>(null);

export function UnsavedChangesProvider({ children }: { children: ReactNode }) {
  const registrationRef = useRef<UnsavedChangesRegistration | null>(null);

  const register = useCallback(
    (guard: UnsavedChangesRegistration | null) => {
      registrationRef.current = guard;
    },
    [],
  );

  const guardNavigation = useCallback((event?: NavigationEvent) => {
    const guard = registrationRef.current;
    if (!guard?.isDirty || guard.confirmDiscard()) return true;
    event?.preventDefault();
    return false;
  }, []);

  useEffect(() => {
    const handleBeforeUnload = (event: BeforeUnloadEvent) => {
      if (!registrationRef.current?.isDirty) return;
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", handleBeforeUnload);
    return () => window.removeEventListener("beforeunload", handleBeforeUnload);
  }, []);

  const value = useMemo(
    () => ({ register, guardNavigation }),
    [guardNavigation, register],
  );

  return (
    <UnsavedChangesGuardContext.Provider value={value}>
      {children}
    </UnsavedChangesGuardContext.Provider>
  );
}

export function useUnsavedChangesGuard() {
  const value = useContext(UnsavedChangesGuardContext);
  if (!value) {
    throw new Error(
      "useUnsavedChangesGuard 必须在 UnsavedChangesProvider 内使用",
    );
  }
  return value;
}
