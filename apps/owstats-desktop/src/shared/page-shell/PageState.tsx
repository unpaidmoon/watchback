export type PageLoadingStateProps = {
  message: string;
};

export type PageEmptyStateProps = {
  title: string;
  description: string;
  actionLabel: string;
  onAction: () => void;
};

export type PageErrorStateProps = {
  message: string;
  actionLabel?: string;
  onRetry: () => void;
};

export function PageLoadingState({ message }: PageLoadingStateProps) {
  return (
    <section className="pageState pageState--loading" role="status" aria-live="polite" aria-busy="true">
      <p>{message}</p>
    </section>
  );
}

export function PageEmptyState({ title, description, actionLabel, onAction }: PageEmptyStateProps) {
  return (
    <section className="pageState pageState--empty">
      <h2>{title}</h2>
      <p>{description}</p>
      <button className="pageStateAction" type="button" onClick={onAction}>
        {actionLabel}
      </button>
    </section>
  );
}

export function PageErrorState({ message, actionLabel = "重试", onRetry }: PageErrorStateProps) {
  return (
    <section className="pageState pageState--error" role="alert">
      <p>{message}</p>
      <button className="pageStateAction" type="button" onClick={onRetry}>
        {actionLabel}
      </button>
    </section>
  );
}
