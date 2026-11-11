import { nexusIcons, type NexusIconName } from "../brand/icons";

export type { NexusIconName };

/** One Nexus icon. Decorative unless `title` is given; it takes the colour of the text around it. */
export function NxIcon({ name, size = 20, title, className = "" }: { name: NexusIconName; size?: number; title?: string; className?: string }) {
  return (
    <svg
      className={`nx-icon ${className}`.trim()}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="currentColor"
      role={title ? "img" : undefined}
      aria-label={title}
      aria-hidden={title ? undefined : true}
    >
      <path fillRule="evenodd" d={nexusIcons[name]} />
    </svg>
  );
}
