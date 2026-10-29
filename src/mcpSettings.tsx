import { useEffect, useState } from "react";
import { mcpInfo, McpStatus } from "./lib/invoke";

const MASK = "•".repeat(16);

/** MCP connection details for an agent: URL, token (masked) and the `claude mcp add` command. */
export default function McpSettings() {
  const [s, setS] = useState<McpStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [reveal, setReveal] = useState(false);
  const [note, setNote] = useState<string | null>(null);

  useEffect(() => {
    mcpInfo().then(setS, (e) => setError(String(e instanceof Error ? e.message : e)));
  }, []);

  if (error) return <p className="error">{error}</p>;
  if (!s) return null;
  if (s.status === "starting") return <p className="muted">MCP server is starting.</p>;
  if (s.status === "failed") return <p className="error">MCP server failed to start: {s.reason}</p>;

  const command = (token: string) =>
    `claude mcp add --transport http monax ${s.url} --header "Authorization: Bearer ${token}"`;
  const copy = (text: string) =>
    navigator.clipboard.writeText(text).then(
      () => setNote("Copied"),
      () => {
        setReveal(true);
        setNote("Clipboard unavailable: select the text and copy it.");
      },
    );
  const shown = reveal ? s.token : MASK;

  return (
    <section>
      <h2>MCP Server</h2>
      <p className="muted">Running. Agents connect to this URL with the bearer token.</p>
      {s.port_changed && <p className="error" role="alert">Port changed - re-run the connection command.</p>}
      <p>URL: <code>{s.url}</code></p>
      <p>
        Token: <code>{shown}</code>{" "}
        <button type="button" onClick={() => setReveal(!reveal)}>{reveal ? "Hide" : "Reveal"}</button>{" "}
        <button type="button" onClick={() => void copy(s.token)}>Copy Token</button>
      </p>
      <p>Claude Code:</p>
      <pre className="snippet">{command(shown)}</pre>
      <button type="button" onClick={() => void copy(command(s.token))}>Copy Command</button>
      {note && <p className="muted" role="status">{note}</p>}
    </section>
  );
}
