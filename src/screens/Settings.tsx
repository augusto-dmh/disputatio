import { type FormEvent, useEffect, useState } from "react";
import { commands } from "../bindings";

// The v1 tracks in queue-rotation order (specs/queue-slice/requirements.md:
// fundamentos-enterprise, system-design, videos; design.md: fundamentos →
// system-design → video).
const TRACKS = ["fundamentos-enterprise", "system-design", "videos"];

// A track's position override is the vault_path of the chunk the track is
// currently at (design.md chunk identity). Blank clears the override; an
// empty vault-path input leaves the stored path untouched.
function Settings() {
  const [vaultPath, setVaultPath] = useState("");
  const [positions, setPositions] = useState<Record<string, string>>({});
  const [status, setStatus] = useState<string | null>(null);

  useEffect(() => {
    commands.getSettings().then((result) => {
      if (result.status === "error") {
        setStatus(`Failed to load settings: ${result.error}`);
        return;
      }
      setVaultPath(result.data.vault_path ?? "");
      setPositions(result.data.track_positions);
    });
  }, []);

  function setPosition(track: string, value: string) {
    setPositions((current) => ({ ...current, [track]: value }));
  }

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    try {
      const result = await commands.setSettings({
        vault_path: vaultPath.trim() === "" ? null : vaultPath.trim(),
        track_positions: positions,
      });
      if (result.status === "error") {
        setStatus(`Failed to save settings: ${result.error}`);
        return;
      }
      setVaultPath(result.data.vault_path ?? "");
      setPositions(result.data.track_positions);
      setStatus("Settings saved.");
    } catch (error) {
      setStatus(`Failed to save settings: ${error}`);
    }
  }

  return (
    <section aria-label="Settings">
      <h2>Settings</h2>
      <form onSubmit={save}>
        <div>
          <label htmlFor="vault-path">Vault path</label>
          <input
            id="vault-path"
            value={vaultPath}
            onChange={(e) => setVaultPath(e.currentTarget.value)}
            placeholder="/path/to/your/vault"
          />
        </div>

        <h3>Track positions</h3>
        {TRACKS.map((track) => (
          <div key={track}>
            <label htmlFor={`position-${track}`}>{track}</label>
            <input
              id={`position-${track}`}
              value={positions[track] ?? ""}
              onChange={(e) => setPosition(track, e.currentTarget.value)}
              placeholder={`Vault path of the chunk ${track} is at (blank = no override)`}
            />
          </div>
        ))}

        <button type="submit">Save</button>
      </form>
      <p role="status">{status}</p>
    </section>
  );
}

export default Settings;
