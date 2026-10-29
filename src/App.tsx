import { useEffect, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import PickPath, { pickerFor } from "./components/PickPath";
import McpSettings from "./mcpSettings";
import FillForm from "./panels/FillForm";
import Organize from "./panels/Organize";
import ToolPanel from "./panels/ToolPanel";
import Viewer from "./panels/Viewer";
import { OFFICE_MISSING } from "./convertTools";
import { officeAvailable } from "./lib/invoke";
import { acceptedFiles, GROUPS, TOOLS, type ToolEntry } from "./tools";
import "./App.css";

export default function App() {
  const [toolId, setToolId] = useState("organize");
  const [files, setFiles] = useState<string[]>([]);
  const [settings, setSettings] = useState(false);
  const [pickError, setPickError] = useState<string | null>(null);
  const [office, setOffice] = useState(true); // until asked: no flash of greyed tools
  /** Why `t` cannot run here, if it cannot. */
  const blocked = (t: ToolEntry) => (t.requires === "office" && !office ? OFFICE_MISSING : undefined);
  const tool = TOOLS.find((t) => t.id === toolId) ?? TOOLS[0];
  const Custom = { "fill-form": FillForm, viewer: Viewer }[tool.id] ?? Organize;
  const mine = acceptedFiles(tool, files);

  useEffect(() => {
    officeAvailable().then(setOffice, () => setOffice(false));
  }, []);

  useEffect(() => {
    // Drag-and-drop gives absolute paths, like the Open button.
    const un = getCurrentWebview().onDragDropEvent((e) => {
      if (e.payload.type === "drop") setFiles(e.payload.paths); // each tool takes the kinds it accepts
    });
    // An agent's pdf_open_in_app: the backend validated the path; it opens in the viewer.
    const unOpen = listen<string>("open-document", (e) => {
      setFiles([e.payload]);
      setToolId("viewer");
      setSettings(false);
    });
    return () => {
      void un.then((f) => f());
      void unOpen.then((f) => f());
    };
  }, []);

  return (
    <div className="app">
      <nav aria-label="Tools">
        {GROUPS.map((g) => {
          const tools = TOOLS.filter((t) => t.group === g);
          return (
            <div key={g}>
              <h3>{g}</h3>
              {tools.length === 0 && <p className="muted">Coming soon</p>}
              {tools.map((t) => (
                <button key={t.id} type="button" aria-current={!settings && t.id === toolId}
                  aria-disabled={blocked(t) ? true : undefined} title={blocked(t)} onClick={() => { setToolId(t.id); setSettings(false); }}>{t.title}</button>
              ))}
            </div>
          );
        })}
        <h3>Settings</h3>
        <button type="button" aria-current={settings} onClick={() => setSettings(true)}>MCP Settings</button>
      </nav>
      <main>
        {!settings && <PickPath {...pickerFor(tool)} onPick={(p) => { setPickError(null); setFiles(p); }} onError={setPickError} />}
        {!settings && pickError && <p role="alert" className="error">{pickError}</p>}
        {settings ? (
          <McpSettings />
        ) : tool.custom ? (
          mine[0] ? <Custom key={tool.id + mine[0]} path={mine[0]} /> : <p className="muted">Drop a PDF onto the window to start.</p>
        ) : (
          <ToolPanel key={tool.id} tool={tool} files={tool.multi ? mine : mine.slice(0, 1)} blocked={blocked(tool)} />
        )}
      </main>
    </div>
  );
}
