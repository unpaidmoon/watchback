import {
  PLAYER_QUERY_MODES,
  type PlayerQueryMode,
} from "./playerQueryTypes";

export { PLAYER_QUERY_MODES, type PlayerQueryMode } from "./playerQueryTypes";

const knownModes = new Set<string>(PLAYER_QUERY_MODES);

/** Normalize untrusted persisted settings without tying them to an account. */
export function normalizeModeOrder(order: readonly string[] | null | undefined): PlayerQueryMode[] {
  const normalized: PlayerQueryMode[] = [];
  for (const mode of order ?? []) {
    if (knownModes.has(mode) && !normalized.includes(mode as PlayerQueryMode)) {
      normalized.push(mode as PlayerQueryMode);
    }
  }
  for (const mode of PLAYER_QUERY_MODES) {
    if (!normalized.includes(mode)) normalized.push(mode);
  }
  return normalized;
}

/**
 * Mouse-drop semantics: move the dragged mode to the target mode's position.
 * Invalid payloads and self-drops are no-ops.
 */
export function reorderModesForDrop(
  current: readonly string[],
  draggedMode: string,
  targetMode: string,
): PlayerQueryMode[] {
  const normalized = normalizeModeOrder(current);
  const from = normalized.indexOf(draggedMode as PlayerQueryMode);
  const to = normalized.indexOf(targetMode as PlayerQueryMode);
  if (from < 0 || to < 0 || from === to) return normalized;

  const next = [...normalized];
  const [dragged] = next.splice(from, 1);
  next.splice(to, 0, dragged);
  return next;
}

export type ModeOrderSaveResult = {
  order: PlayerQueryMode[];
  changed: boolean;
  saveError: unknown | null;
};

/**
 * Apply a mouse drop and save immediately only when it changes the order.
 * A failed save intentionally returns the new in-memory order together with
 * the error so the UI can keep the reorder visible and report the failure.
 */
export async function saveModeOrderAfterDrop(
  current: readonly string[],
  draggedMode: string,
  targetMode: string,
  save: (order: readonly PlayerQueryMode[]) => Promise<unknown>,
): Promise<ModeOrderSaveResult> {
  const before = normalizeModeOrder(current);
  const order = reorderModesForDrop(before, draggedMode, targetMode);
  const changed = order.some((mode, index) => mode !== before[index]);
  if (!changed) return { order, changed: false, saveError: null };

  try {
    await save(order);
    return { order, changed: true, saveError: null };
  } catch (error) {
    return { order, changed: true, saveError: error };
  }
}
