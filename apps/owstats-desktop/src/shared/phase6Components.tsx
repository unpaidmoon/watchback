import { useEffect, useState, type ImgHTMLAttributes, type ReactNode } from "react";
import { heroAssetUrls, mapAssetUrls } from "./assets";

type GlassPanelProps = {
  children: ReactNode;
  className?: string;
  tone?: "soft" | "solid";
};

type AssetImageProps = Omit<ImgHTMLAttributes<HTMLImageElement>, "src"> & {
  sources: string[];
  fallbackLabel: string;
};

type HeroAvatarProps = {
  heroKey?: string | null;
  name: string;
  size?: "sm" | "md";
};

type MapPreviewProps = {
  mapKey?: string | null;
  name: string;
  className?: string;
  decorative?: boolean;
};

export function GlassPanel({ children, className = "", tone = "soft" }: GlassPanelProps) {
  return <section className={`phase6GlassPanelShared ${tone === "solid" ? "solid" : ""} ${className}`}>{children}</section>;
}

function AssetImage({ sources, fallbackLabel, className = "", ...imageProps }: AssetImageProps) {
  const [sourceIndex, setSourceIndex] = useState(0);
  const src = sources[sourceIndex] ?? null;

  useEffect(() => {
    setSourceIndex(0);
  }, [sources.join("|")]);

  if (!src) {
    return (
      <span
        className={`phase6AssetFallback ${className}`}
        aria-label={imageProps["aria-hidden"] ? undefined : `${fallbackLabel}暂无图片`}
        aria-hidden={imageProps["aria-hidden"]}
      />
    );
  }

  return (
    <img
      {...imageProps}
      className={className}
      src={src}
      alt={imageProps.alt ?? fallbackLabel}
      loading={imageProps.loading ?? "lazy"}
      onError={() => {
        if (sourceIndex + 1 < sources.length) {
          setSourceIndex((value) => value + 1);
        } else {
          setSourceIndex(sources.length);
        }
      }}
    />
  );
}

export function HeroAvatar({ heroKey, name, size = "md" }: HeroAvatarProps) {
  return <AssetImage className={`phase6HeroAvatarShared ${size}`} sources={heroAssetUrls([heroKey, name])} fallbackLabel={name} alt={name} />;
}

export function MapPreview({ mapKey, name, className = "", decorative = false }: MapPreviewProps) {
  return (
    <AssetImage
      className={`phase6MapPreviewShared ${className}`}
      sources={mapAssetUrls([mapKey, name])}
      fallbackLabel={name}
      alt={decorative ? "" : name}
      aria-hidden={decorative ? "true" : undefined}
    />
  );
}
