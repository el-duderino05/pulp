// SPDX-License-Identifier: MPL-2.0
import { useEffect, useState } from "react";
import { startup, type Startup } from "./app/core";
import { friendlyMessage } from "./app/errors";
import { handleShortcut } from "./app/commands";
import { t } from "./i18n/en";

// Stubs until Tasks 8-11 replace them with the real screens.
const Setup = ({ defaultPath }: { defaultPath: string }) => <main>Setup: {defaultPath}</main>;
const LocateLibrary = ({ path }: { path: string }) => <main>{t("library.missing", { path })}</main>;
const Library = () => <main>Library</main>;

export default function App() {
  const [s, setS] = useState<Startup | null>(null);
  const [err, setErr] = useState<string | null>(null);

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
      return <Setup defaultPath={s.defaultPath} />;
    case "libraryMissing":
      return <LocateLibrary path={s.path} />;
    case "ready":
      return <Library />;
    case "failed":
      return <main role="alert">{friendlyMessage(s.error)}</main>;
  }
}
