import { useCallback, useEffect, useState } from "react";
import { type CardDto, commands, type QueueDto } from "../bindings";

// Home is the Daily Queue (specs/queue-slice/requirements.md): per track the
// chunks to consider today — stale flagged ahead of new ones — and the due
// header counting the app's own FSRS backlog (specs/memoria-slice: since 0.2
// scheduling is internal; AnkiConnect is gone from the queue path). The
// Memoria block reviews due cards inline: front → reveal → grade. Sessions
// log through Rust only (AGENTS.md boundary): everything via the generated
// bindings.
type ActiveSession = { sessionId: number; chunkId: number };

const GRADES = ["again", "hard", "good", "easy"] as const;

// One review sitting: a bounded batch of due cards (requirements.md EARS).
type Review = {
  batch: CardDto[];
  index: number;
  revealed: boolean;
  shownAt: number;
};

function Home() {
  const [queue, setQueue] = useState<QueueDto | null>(null);
  const [active, setActive] = useState<ActiveSession | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [review, setReview] = useState<Review | null>(null);

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
    const logged =
      seconds == null ? "" : seconds < 60 ? `${seconds}s` : `${Math.round(seconds / 60)} min`;
    setStatus(seconds == null ? "Session stopped." : `Session stopped — ${logged} logged.`);
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

  // Opens a review sitting: one bounded batch of due cards, oldest first.
  async function startReview() {
    const result = await commands.listDueCards(null);
    if (result.status === "error") {
      setStatus(`Failed to load reviews: ${result.error}`);
      return;
    }
    if (result.data.length === 0) {
      setStatus("Nothing due — reviews are clear.");
      return;
    }
    setStatus(null);
    setReview({ batch: result.data, index: 0, revealed: false, shownAt: Date.now() });
  }

  function reveal() {
    setReview((r) => (r == null ? r : { ...r, revealed: true }));
  }

  // Grades the current card and advances; the last grade closes the sitting
  // and refreshes the queue so the due header reflects the work.
  async function grade(g: (typeof GRADES)[number]) {
    if (review == null) {
      return;
    }
    const card = review.batch[review.index];
    const result = await commands.gradeCard(card.id, g, Date.now() - review.shownAt);
    if (result.status === "error") {
      setStatus(`Failed to grade: ${result.error}`);
      return;
    }
    const next = review.index + 1;
    if (next < review.batch.length) {
      setReview({ ...review, index: next, revealed: false, shownAt: Date.now() });
      return;
    }
    setReview(null);
    setStatus(`Sitting done — ${review.batch.length} graded. FSRS scheduled the rest.`);
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

  const card = review == null ? null : review.batch[review.index];

  return (
    <section aria-label="Daily Queue">
      <h2>Daily Queue</h2>
      <p>{queue == null ? "Loading…" : `Due reviews: ${queue.due_total}`}</p>

      {review == null ? (
        <button type="button" onClick={startReview} disabled={queue == null}>
          Review now
        </button>
      ) : (
        card != null && (
          <section aria-label="Review">
            <h3>
              Memoria — card {review.index + 1} of {review.batch.length}
            </h3>
            <p>
              <strong>{card.front}</strong>
            </p>
            {review.revealed ? (
              <>
                <p>{card.back}</p>
                {GRADES.map((g) => (
                  <button key={g} type="button" aria-label={`Grade ${g}`} onClick={() => grade(g)}>
                    {g[0].toUpperCase() + g.slice(1)}
                  </button>
                ))}
              </>
            ) : (
              <button type="button" onClick={reveal}>
                Reveal
              </button>
            )}
          </section>
        )
      )}

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
