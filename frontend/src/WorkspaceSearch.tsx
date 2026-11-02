import { useEffect, useMemo, useRef, useState } from "react";
import type { Bootstrap } from "./api";
import { Icon, Modal } from "./ui";
import { searchWorkspace } from "./search";
import type { SearchResult } from "./search";
import "./search.css";

export default function WorkspaceSearch({ data, onSelect }: { data: Bootstrap; onSelect: (result: SearchResult) => void }) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const input = useRef<HTMLInputElement>(null);
  const results = useMemo(() => searchWorkspace(data, query), [data, query]);
  const close = () => { setOpen(false); setQuery(""); };
  useEffect(() => {
    const keydown = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k" && !document.querySelector("dialog[open]")) {
        event.preventDefault(); setOpen(true);
      }
    };
    window.addEventListener("keydown", keydown);
    return () => window.removeEventListener("keydown", keydown);
  }, []);
  useEffect(() => { if (open) input.current?.focus(); }, [open]);
  return <>
    <button type="button" className="workspace-search-trigger" onClick={() => setOpen(true)} aria-label="Search workspace">
      <Icon name="search" size={17} /><span>Search workspace</span><kbd>Ctrl K</kbd>
    </button>
    {open && <Modal title="Search your workspace" description="Find clients, appointments, documentation, financial records, and team members available to your role." onClose={close}>
      <div className="workspace-search-body">
        <label className="sr-only" htmlFor="global-workspace-query">Search records</label>
        <input ref={input} id="global-workspace-query" type="search" autoComplete="off" placeholder="Name, task, claim reference, or form…" value={query} onChange={event => setQuery(event.target.value)} />
        <p className="workspace-search-count" role="status">{query.trim() ? `${results.length === 30 ? "First 30" : results.length} matching records` : "Type to search. Use Tab to move through the results."}</p>
        <ul className="workspace-search-results">
          {results.map(result => <li key={result.key}><button type="button" onClick={() => { close(); onSelect(result); }}>
            <span><strong>{result.title}</strong><small>{result.detail.replaceAll("_", " ")}</small></span><Icon name="arrow" size={17} />
          </button></li>)}
        </ul>
        {query.trim() && !results.length && <p className="workspace-search-empty">No matches in the records available to you. Try a shorter name or reference.</p>}
      </div>
    </Modal>}
  </>;
}
