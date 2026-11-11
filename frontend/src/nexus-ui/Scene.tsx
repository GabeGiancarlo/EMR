import { useId } from "react";
import { nexusArt, type NexusSceneName } from "../brand/art";

export type { NexusSceneName };

/** A cut-paper scene. Its colours follow the theme: daylight in light, moonlit in dark. */
export function Scene({ name, className = "", title }: { name: NexusSceneName; className?: string; title?: string }) {
  return (
    <svg
      className={`nx-scene ${className}`.trim()}
      viewBox={nexusArt.sceneViewBox}
      role={title ? "img" : undefined}
      aria-label={title}
      aria-hidden={title ? undefined : true}
      dangerouslySetInnerHTML={{ __html: nexusArt.scenes[name] }}
    />
  );
}

/** The contour pattern, tiled as vector. Place it in a positioned, clipped box; it takes `color`. */
export function ContourPattern({ tile = 420, className = "" }: { tile?: number; className?: string }) {
  const id = useId().replace(/[^a-zA-Z0-9_-]/g, "");
  const scale = tile / nexusArt.contour.size;
  return (
    <svg className={`nx-pattern ${className}`.trim()} width="100%" height="100%" aria-hidden="true">
      <defs>
        <pattern
          id={`nx-contour-${id}`}
          width={nexusArt.contour.size}
          height={nexusArt.contour.size}
          patternUnits="userSpaceOnUse"
          patternTransform={`scale(${scale})`}
          dangerouslySetInnerHTML={{ __html: nexusArt.contour.markup }}
        />
      </defs>
      <rect width="100%" height="100%" fill={`url(#nx-contour-${id})`} />
    </svg>
  );
}
