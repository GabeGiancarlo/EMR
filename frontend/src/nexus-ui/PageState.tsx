import { Component, type ReactNode } from "react";
import { isLoadFailure } from "../pageFailure.ts";
import { NxButton } from "./components";
import { Logo } from "./Logo";

/** What a whole page shows while its code is still arriving. */
export function PageLoading({ label = "Loading Nexus…" }: { label?: string }) {
  return (
    <div className="nx-page-state" role="status">
      <Logo size={34} />
      <span className="nx-page-spinner" aria-hidden="true" />
      <p>{label}</p>
    </div>
  );
}

type Failure = "none" | "broke" | "not-loaded";

/**
 * The last line of defence for a page: without it, any error while drawing leaves a blank
 * screen. It says plainly what happened and offers a reload. The error's own text is for
 * the console, never for the person looking at the page.
 */
export class ErrorBoundary extends Component<{ children: ReactNode }, { failure: Failure }> {
  state: { failure: Failure } = { failure: "none" };

  static getDerivedStateFromError(error: unknown): { failure: Failure } {
    return { failure: isLoadFailure(error) ? "not-loaded" : "broke" };
  }

  render() {
    const { failure } = this.state;
    if (failure === "none") return this.props.children;
    return (
      <main className="nx-page-state" role="alert">
        <Logo size={34} />
        <h1>{failure === "not-loaded" ? "Nexus did not finish loading." : "Something went wrong."}</h1>
        <p>
          {failure === "not-loaded"
            ? "Part of this page could not be downloaded. Check your connection, then reload."
            : "This page hit a problem, and anything you typed may not have been saved."}
        </p>
        <NxButton icon="refresh" onClick={() => window.location.reload()}>
          Reload
        </NxButton>
      </main>
    );
  }
}
