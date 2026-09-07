import { useCallback, useEffect, useState } from "react";
import { commands, type QueueDto } from "../bindings";

// Home is the Daily Queue (specs/queue-slice/requirements.md): per track the
// chunks to consider today — stale flagged ahead of new ones — and the Anki
// due total (or offline note) in the header. Sessions log through Rust only
// (AGENTS.md boundary): start/stop/complete via the generated bindings.
type ActiveSession = { sessionId: number; chunkId: number };

function Home() {
  const [queue, setQueue] = useState<QueueDto | null>(null);
  const [active, setActive] = useState<ActiveSession | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadQueue = useCallback(async () => {
    const result = await commands.getQueue();
    if (result.status === "error") {
      setError(`Failed to load the queue: ${result.error}`);
      return;
    }
    setError(null);
    setQueue(result.data);
  }, []);

  useEffect(() => {
    loadQueue();
    commands.getActiveSession().then((result) => {
      if (result.status === "ok" && result.data) {
        setActive({
          sessionId: result.data.session_id,
          chunkId: result.data.chunk_id,
        });
      }
    });
  }, [loadQueue]);

  async function startSession(chunkId: number, title: string) {
    const result = await commands.startSession(chunkId);
    if (result.status === "error") {
      setStatus(`Failed to start: ${result.error}`);
      return;
    }
    setActive({ sessionId: result.data.session_id, chunkId: result.data.chunk_id });
    setStatus(`Session started — studying “${title}”.`);
  }

  async function stopSession() {
    const result = await commands.stopSession();
    if (result.status === "error") {
      setStatus(`Failed to stop: ${result.error}`);
      return;
    }
    setActive(null);
    const seconds = result.data.duration_seconds;
    setStatus(
      seconds != null
        ? `Session stopped — ${Math.round(seconds / 60)} min logged.`
        : "Session stopped.",
    );
  }

  async function completeChunk(chunkId: number, title: string) {
    const result = await commands.completeChunk(chunkId);
    if (result.status === "error") {
      setStatus(`Failed to complete: ${result.error}`);
      return;
    }
    if (active?.chunkId === chunkId) {
      setActive(null);
    }
    setStatus(`“${title}” complete — the next chunk in line takes over.`);
    await loadQueue();
  }

  if (error) {
    return (
      <section aria-label="Daily Queue">
        <h2>Daily Queue</h2>
        <p role="alert">{error}</p>
      </section>
    );
  }

  return (
    <section aria-label="Daily Queue">
      <h2>Daily Queue</h2>
      <p>
        {queue == null
          ? "Loading…"
          : queue.anki.due_total != null
            ? `Anki due: ${queue.anki.due_total}`
            : queue.anki.note}
      </p>

      {queue != null && queue.tracks.length === 0 && (
        <p>Nothing queued — index your vault from Settings.</p>
      )}

      {queue?.tracks.map((track) => (
        <section key={track.track} aria-label={`Queue: ${track.track}`}>
          <h3>{track.track}</h3>
          <ul>
            {track.items.map((item) => {
              const isActive = active?.chunkId === item.chunk_id;
              return (
                <li key={item.chunk_id}>
                  {item.stale && <strong>Stale — </strong>}
                  {item.title} <em>({item.status === "in_progress" ? "in progress" : "queued"})</em>
                  {isActive ? (
                    <button type="button" onClick={() => stopSession()}>
                      Stop session
                    </button>
                  ) : (
                    <button
                      type="button"
                      aria-label={`Start session ${item.title}`}
                      disabled={active != null}
                      onClick={() => startSession(item.chunk_id, item.title)}
                    >
                      Start session
                    </button>
                  )}
                  <button
                    type="button"
                    aria-label={`Complete ${item.title}`}
                    onClick={() => completeChunk(item.chunk_id, item.title)}
                  >
                    Complete
                  </button>
                </li>
              );
            })}
          </ul>
        </section>
      ))}

      <p role="status">{status}</p>
    </section>
  );
}

export default Home;
