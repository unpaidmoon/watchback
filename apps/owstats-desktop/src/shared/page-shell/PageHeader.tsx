import type { ReactNode } from "react";

export type PageHeaderProps = {
  title: string;
  description: string;
  actions?: ReactNode;
  className?: string;
};

export function PageHeader({ title, description, actions, className }: PageHeaderProps) {
  const classes = ["pageHeader", className].filter(Boolean).join(" ");

  return (
    <header className={classes}>
      <div className="pageHeaderContent">
        <h1>{title}</h1>
        <p>{description}</p>
      </div>
      {actions ? <div className="pageHeaderActions">{actions}</div> : null}
    </header>
  );
}
