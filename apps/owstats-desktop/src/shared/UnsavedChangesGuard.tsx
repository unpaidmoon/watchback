import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  type ReactNode,
} from "react";

export type UnsavedChangesGuardRegistration = {
  isDirty: boolean;
  isBusy?: boolean;
  confirmDiscard: () => boolean;
};

type NavigationEvent = {
  preventDefault: () => void;
};

type BeforeUnloadEventLike = NavigationEvent & {
  returnValue: string;
};

type UnsavedChangesGuardValue = {
  register: (guard: UnsavedChangesGuardRegistration | null) => void;
  guardNavigation: (event?: NavigationEvent) => boolean;
};

const UnsavedChangesGuardContext =
  createContext<UnsavedChangesGuardValue | null>(null);

export function guardUnsavedNavigation(
  registration: UnsavedChangesGuardRegistration | null,
  event?: NavigationEvent,
) {
  if (registration?.isBusy) {
    event?.preventDefault();
    return false;
  }
  if (!registration?.isDirty || registration.confirmDiscard()) return true;
  event?.preventDefault();
  return false;
}

export function guardUnsavedBeforeUnload(
  registration: UnsavedChangesGuardRegistration | null,
  event: BeforeUnloadEventLike,
) {
  if (!registration?.isDirty && !registration?.isBusy) return;
  event.preventDefault();
  event.returnValue = "";
}

export function UnsavedChangesProvider({ children }: { children: ReactNode }) {
  const registrationRef = useRef<UnsavedChangesGuardRegistration | null>(null);

  const register = useCallback(
    (guard: UnsavedChangesGuardRegistration | null) => {
      registrationRef.current = guard;
    },
    [],
  );

  const guardNavigation = useCallback(
    (event?: NavigationEvent) =>
      guardUnsavedNavigation(registrationRef.current, event),
    [],
  );

  useEffect(() => {
    const handleBeforeUnload = (event: BeforeUnloadEvent) =>
      guardUnsavedBeforeUnload(registrationRef.current, event);
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
    throw new Error("UnsavedChangesGuardProvider is missing");
  }
  return value;
}
