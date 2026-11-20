// SPDX-License-Identifier: MPL-2.0
import { useEffect, useState } from "react";
import { startup, type LibraryInfo, type OpenedProject, type Startup } from "./app/core";
import { friendlyMessage } from "./app/errors";
import { handleShortcut } from "./app/commands";
import Setup from "./screens/Setup";
import LocateLibrary from "./screens/LocateLibrary";
import Library from "./screens/Library";
import Project from "./screens/Project";

export default function App() {
  const [s, setS] = useState<Startup | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [project, setProject] = useState<OpenedProject | null>(null);
  const done = (library: LibraryInfo) => setS({ state: "ready", library });

  useEffect(() => {
    startup().then(setS, (e) => setErr(friendlyMessage(e)));
    const onKey = (e: KeyboardEvent) => {
      if (handleShortcut(e)) e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  if (err) return <main role="alert">{err}</main>;
  if (!s) return null;
  switch (s.state) {
    case "firstLaunch":
      return <Setup defaultPath={s.defaultPath} onDone={done} />;
    case "libraryMissing":
      return <LocateLibrary path={s.path} onDone={done} />;
    case "ready":
      return project ? <Project opened={project} onBack={() => setProject(null)} /> : <Library info={s.library} onOpen={setProject} />;
    case "failed":
      return <LocateLibrary path="" message={friendlyMessage(s.error)} onDone={done} />;
  }
}
