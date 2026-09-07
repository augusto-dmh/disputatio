import { useState } from "react";
import "./App.css";
import Home from "./screens/Home";
import Settings from "./screens/Settings";

type Screen = "home" | "settings";

const SCREENS: { id: Screen; label: string }[] = [
  { id: "home", label: "Home" },
  { id: "settings", label: "Settings" },
];

function App() {
  const [screen, setScreen] = useState<Screen>("home");

  return (
    <main className="container">
      <nav className="row" aria-label="Screens">
        {SCREENS.map(({ id, label }) => (
          <button
            key={id}
            type="button"
            onClick={() => setScreen(id)}
            aria-current={screen === id ? "page" : undefined}
          >
            {label}
          </button>
        ))}
      </nav>
      {screen === "home" ? <Home /> : <Settings />}
    </main>
  );
}

export default App;
