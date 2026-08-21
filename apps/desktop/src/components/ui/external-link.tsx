import { openUrl } from "@/lib/open-url";

interface ExternalLinkProps {
  href: string;
  children: React.ReactNode;
  className?: string;
}

export function ExtLink({ href, children, className }: ExternalLinkProps) {
  return (
    <span
      role="link"
      tabIndex={0}
      onClick={(e) => {
        e.preventDefault();
        e.stopPropagation();
        openUrl(href);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          openUrl(href);
        }
      }}
      className={className ?? "ext-link"}>
      {children}
    </span>
  );
}
