import { useState } from "react";
import type { FormEvent } from "react";
import { request } from "./api";
import type { Row } from "./api";
import { Brand, Button, Icon } from "./ui";
export default function Setup({ onDone }: { onDone: (id: string) => void }) {
  const [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  async function submit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setError("");
    setBusy(true);
    const body = Object.fromEntries(new FormData(e.currentTarget));
    try {
      const result = await request<{ actor: Row }>("/api/setup", "POST", body);
      onDone(result.actor.id);
    } catch (e) {
      setError(
        e instanceof Error ? e.message : "Unable to set up your workspace.",
      );
    } finally {
      setBusy(false);
    }
  }
  return (
    <div className="setup-page">
      <section className="setup-story">
        <Brand />
        <div className="setup-story-content">
          <span className="eyebrow light">CARE, CONNECTED.</span>
          <h1>
            More connection.
            <br />
            More room for care.
          </h1>
          <p>
            Bring your clinical team, operations, and financial picture
            together. Start with a workspace that belongs to you.
          </p>
          <div className="orbit-art" aria-hidden="true">
            <div className="orbit outer" />
            <div className="orbit inner" />
            <div className="orbit-center">
              <Icon name="leaf" size={42} />
            </div>
            <span className="orbit-point one">
              <Icon name="clients" />
            </span>
            <span className="orbit-point two">
              <Icon name="notes" />
            </span>
            <span className="orbit-point three">
              <Icon name="billing" />
            </span>
          </div>
        </div>
        <p className="setup-caption">
          Built around people. Designed for recovery.
        </p>
      </section>
      <section className="setup-form-wrap">
        <span className="setup-step">
          01 <span>/ YOUR FOUNDATION</span>
        </span>
        <h2>Welcome to Nexus.</h2>
        <p>
          Let’s create a home for your organization.
          <br />
          Your workspace starts empty, ready for your own records.
        </p>
        <form onSubmit={submit}>
          <div className="form-field">
            <label htmlFor="org">Organization name</label>
            <input
              id="org"
              name="organization_name"
              required
              autoFocus
              placeholder="Your organization"
              maxLength={200}
            />
          </div>
          <div className="form-field">
            <label htmlFor="facility">First facility</label>
            <input
              id="facility"
              name="facility_name"
              required
              placeholder="Facility or practice name"
              maxLength={200}
            />
          </div>
          <div className="form-grid">
            <div className="form-field">
              <label htmlFor="admin-name">Your name</label>
              <input
                id="admin-name"
                name="admin_name"
                required
                autoComplete="name"
                maxLength={200}
              />
            </div>
            <div className="form-field">
              <label htmlFor="admin-email">Work email</label>
              <input
                id="admin-email"
                name="admin_email"
                type="email"
                required
                autoComplete="email"
                maxLength={200}
              />
            </div>
          </div>
          <div className="setup-local">
            <Icon name="shield" />
            <div>
              <strong>Local workspace</strong>
              <p>
                Identity-provider sign-in is deferred. This creates your local
                administrator record, not an authenticated account.
              </p>
            </div>
          </div>
          {error && (
            <div className="error-banner" role="alert">
              {error}
            </div>
          )}
          <Button type="submit" className="full-width" disabled={busy}>
            {busy ? "Creating your workspace…" : "Create workspace"}
            <Icon name="arrow" size={18} />
          </Button>
        </form>
      </section>
    </div>
  );
}
