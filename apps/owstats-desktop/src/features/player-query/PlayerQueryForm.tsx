import { useId, useState, type FormEvent, type ReactNode } from "react";
import { isValidBattleTag, normalizeBattleTag } from "./playerQueryState";

export function PlayerQueryForm({ disabled, searching, trailingControl, value, onValueChange, onSubmit }: { disabled: boolean; searching: boolean; trailingControl?: ReactNode; value?: string; onValueChange?: (value: string) => void; onSubmit: (battleTag: string) => void }) {
  const id = useId(); const [internalValue, setInternalValue] = useState(""); const [error, setError] = useState<string | null>(null);
  const currentValue = value ?? internalValue;
  const changeValue = onValueChange ?? setInternalValue;
  const submit = (event: FormEvent) => { event.preventDefault(); const normalized = normalizeBattleTag(currentValue); if (!isValidBattleTag(normalized)) { setError("请输入“名称#数字”格式的 BattleTag"); return; } setError(null); onSubmit(normalized); };
  return <form className="playerQueryForm" onSubmit={submit} noValidate>
    <div className="playerQueryFormControls"><input id={id} aria-label="BattleTag" value={currentValue} disabled={disabled || searching} aria-invalid={Boolean(error)} aria-describedby={error ? `${id}-error` : undefined} placeholder="玩家名称#1234" autoComplete="off" onChange={(event) => changeValue(event.currentTarget.value)} />{trailingControl}<button className="primaryButton" type="submit" disabled={disabled || searching}>{searching ? "正在查询…" : "查询玩家"}</button></div>
    {error ? <p id={`${id}-error`} className="playerQueryInlineError" role="alert">{error}</p> : null}
  </form>;
}
