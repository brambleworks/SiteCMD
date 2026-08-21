import { GitHubLogo, BingLogo, GoogleLogo, CloudflareLogo } from "@/components/icons/BrandLogos";
import { BarChart3, Wifi } from "lucide-react";

const ICON_MAP: Record<string, React.FC<{ className?: string }>> = {
  github: GitHubLogo,
  bingwebmaster: BingLogo,
  googleanalytics: GoogleLogo,
  googlesearchconsole: GoogleLogo,
  cloudflare: CloudflareLogo,
};

const ICON_COLOR_CLASSES: Record<string, string> = {
  plausible: "service-icon--plausible",
  cloudflare: "service-icon--cloudflare",
  uptimerobot: "service-icon--uptimerobot",
  bingwebmaster: "service-icon--bing",
  github: "service-icon--github",
};

const FALLBACK_MAP: Record<string, React.FC<{ className?: string }>> = {
  plausible: BarChart3,
  uptimerobot: Wifi,
};

interface ServiceIconProps {
  type: string;
  className?: string;
}

export function ServiceIcon({ type, className = "icon-lg" }: ServiceIconProps) {
  const BrandIcon = ICON_MAP[type];
  if (BrandIcon) {
    return <BrandIcon className={className} />;
  }
  const Fallback = FALLBACK_MAP[type];
  if (Fallback) {
    return <Fallback className={className} />;
  }
  return <BarChart3 className={className} />;
}

export function ServiceIconWithBg({ type }: { type: string }) {
  const color = ICON_COLOR_CLASSES[type] ?? "text-muted-foreground";
  return (
    <div className={`service-icon-tile ${color}`}>
      <ServiceIcon type={type} />
    </div>
  );
}
