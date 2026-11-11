import { nexusArt } from "../brand/art";

type LogoProps = {
  /** "horizontal" is the default lockup; "symbol" when the name is already nearby. */
  variant?: "horizontal" | "stacked" | "symbol" | "wordmark";
  /** Height of the symbol in pixels. Below 32 the small-size drawing is used. */
  size?: number;
  className?: string;
};

function Symbol({ size }: { size: number }) {
  const art = size < 32 ? nexusArt.symbolSmall : nexusArt.symbol;
  return (
    <svg className="nx-logo-symbol" width={size} height={size} viewBox={art.viewBox} fill="currentColor" aria-hidden="true">
      <path fillRule="evenodd" d={art.d} />
    </svg>
  );
}

function Wordmark({ height }: { height: number }) {
  const [, , w, h] = nexusArt.wordmark.viewBox.split(" ").map(Number);
  return (
    <svg className="nx-logo-wordmark" width={(height * w) / h} height={height} viewBox={nexusArt.wordmark.viewBox} fill="currentColor" aria-hidden="true">
      <path fillRule="evenodd" d={nexusArt.wordmark.d} />
    </svg>
  );
}

/** The Nexus logo, drawn from the brand kit's outlines. One colour: it inherits `color`. */
export function Logo({ variant = "horizontal", size = 32, className = "" }: LogoProps) {
  return (
    <span className={`nx-logo nx-logo--${variant} ${className}`.trim()} role="img" aria-label="Nexus">
      {variant !== "wordmark" && <Symbol size={size} />}
      {variant !== "symbol" && <Wordmark height={variant === "stacked" ? size * 0.31 : variant === "wordmark" ? size : size * 0.46} />}
    </span>
  );
}
