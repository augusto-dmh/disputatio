import { invoke } from "@tauri-apps/api/core";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import Home from "./Home";

// The screen talks to Rust only through invoke (AGENTS.md boundary) — via the
// generated tauri-specta bindings, which call it under the hood.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

// Testing Library's auto-cleanup needs global afterEach; vitest globals are off.
afterEach(cleanup);

beforeEach(() => {
  invokeMock.mockReset();
});

const queueFixture = {
  due_total: 4,
  tracks: [
    {
      track: "fundamentos-enterprise",
      items: [
        {
          chunk_id: 2,
          track: "fundamentos-enterprise",
          title: "Modelos de negócio",
          vault_path: "courses/fundamentos-enterprise/sessions/02-modelos-de-negocio",
          status: "in_progress",
          stale: true,
          est_minutes: null,
        },
        {
          chunk_id: 3,
          track: "fundamentos-enterprise",
          title: "Propósito da empresa",
          vault_path: "courses/fundamentos-enterprise/sessions/03-proposito",
          status: "queued",
          stale: false,
          est_minutes: null,
        },
      ],
    },
    {
      track: "videos",
      items: [
        {
          chunk_id: 9,
          track: "videos",
          title: "Aula inaugural",
          vault_path: "courses/videos/aula-inaugural.mp4",
          status: "queued",
          stale: false,
          est_minutes: null,
        },
      ],
    },
  ],
};

function queueResultOk() {
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd === "get_queue") return Promise.resolve(queueFixture);
    if (cmd === "get_active_session") return Promise.resolve(null);
    return Promise.reject(`unexpected command ${cmd}`);
  });
}

const cardA = {
  id: 101,
  deck: "fundamentos",
  front: "What is ordo?",
  back: "The order of the subject matter.",
  due: "2026-09-07T10:00:00Z",
  state: "review",
  reps: 3,
};
const cardB = {
  id: 102,
  deck: "fundamentos",
  front: "Why narratio before notation?",
  back: "Recall comes before notes.",
  due: "2026-09-07T11:00:00Z",
  state: "review",
  reps: 2,
};

function reviewMocks() {
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd === "get_queue") return Promise.resolve(queueFixture);
    if (cmd === "get_active_session") return Promise.resolve(null);
    if (cmd === "list_due_cards") return Promise.resolve([cardA, cardB]);
    if (cmd === "grade_card") return Promise.resolve({ ...cardA, due: "2026-09-14T10:00:00Z" });
    return Promise.reject(`unexpected command ${cmd}`);
  });
}

describe("Home screen (Daily Queue)", () => {
  it("renders track sections, stale flags and the internal due header", async () => {
    queueResultOk();

    render(<Home />);

    const fundamentos = await screen.findByLabelText("Queue: fundamentos-enterprise");
    expect(await within(fundamentos).findByText("Modelos de negócio")).toBeDefined();
    expect(within(fundamentos).getByText(/Stale —/)).toBeDefined();

    const videos = await screen.findByLabelText("Queue: videos");
    expect(await within(videos).findByText("Aula inaugural")).toBeDefined();

    expect(await screen.findByText("Due reviews: 4")).toBeDefined();
  });

  it("shows a zero backlog as a normal header", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "get_queue") {
        return Promise.resolve({ ...queueFixture, due_total: 0 });
      }
      if (cmd === "get_active_session") return Promise.resolve(null);
      return Promise.reject(`unexpected command ${cmd}`);
    });

    render(<Home />);

    await waitFor(() => expect(screen.getByText("Due reviews: 0")).toBeDefined());
  });

  it("starts a session on a chunk and swaps the button to stop", async () => {
    queueResultOk();
    render(<Home />);
    const fundamentos = await screen.findByLabelText("Queue: fundamentos-enterprise");
    const start = await within(fundamentos).findByRole("button", {
      name: "Start session Modelos de negócio",
    });

    invokeMock.mockResolvedValueOnce({
      session_id: 42,
      chunk_id: 2,
      closed: false,
      duration_seconds: null,
    });
    fireEvent.click(start);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("start_session", { chunkId: 2 });
    });
    const stop = await within(fundamentos).findByRole("button", { name: "Stop session" });
    expect(screen.getByRole("status").textContent).toBe(
      "Session started — studying “Modelos de negócio”.",
    );

    invokeMock.mockResolvedValueOnce({
      session_id: 42,
      chunk_id: 0,
      closed: true,
      duration_seconds: 1500,
    });
    fireEvent.click(stop);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("stop_session");
    });
    await waitFor(() =>
      expect(screen.getByRole("status").textContent).toBe("Session stopped — 25 min logged."),
    );
  });

  it("offers stop right away when a session is already open (restart resume)", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "get_queue") return Promise.resolve(queueFixture);
      if (cmd === "get_active_session") {
        return Promise.resolve({
          session_id: 42,
          chunk_id: 2,
          closed: false,
          duration_seconds: null,
        });
      }
      return Promise.reject(`unexpected command ${cmd}`);
    });

    render(<Home />);

    const fundamentos = await screen.findByLabelText("Queue: fundamentos-enterprise");
    await within(fundamentos).findByRole("button", { name: "Stop session" });
    expect(
      within(fundamentos).queryByRole("button", { name: "Start session Modelos de negócio" }),
    ).toBeNull();
  });

  it("marks a chunk complete and refreshes the queue", async () => {
    queueResultOk();
    render(<Home />);
    const videos = await screen.findByLabelText("Queue: videos");
    const complete = await within(videos).findByRole("button", {
      name: "Complete Aula inaugural",
    });

    invokeMock.mockResolvedValueOnce(null); // complete_chunk
    // The refresh after completion: the completed chunk leaves the queue.
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "get_queue") {
        return Promise.resolve({ due_total: 3, tracks: [] });
      }
      if (cmd === "get_active_session") return Promise.resolve(null);
      return Promise.reject(`unexpected command ${cmd}`);
    });

    fireEvent.click(complete);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("complete_chunk", { chunkId: 9 });
    });
    await waitFor(() => expect(screen.getByRole("status").textContent).toContain("complete"));
    await waitFor(() => {
      expect(screen.getByText("Nothing queued — index your vault from Settings.")).toBeDefined();
    });
  });

  it("reviews a card end to end: load batch, reveal, grade, advance", async () => {
    reviewMocks();
    render(<Home />);

    fireEvent.click(await screen.findByRole("button", { name: "Review now" }));

    const panel = await screen.findByLabelText("Review");
    expect(await within(panel).findByText("What is ordo?")).toBeDefined();
    expect(within(panel).getByText(/card 1 of 2/)).toBeDefined();
    expect(within(panel).queryByText("The order of the subject matter.")).toBeNull();

    fireEvent.click(within(panel).getByRole("button", { name: "Reveal" }));
    expect(await within(panel).findByText("The order of the subject matter.")).toBeDefined();

    fireEvent.click(within(panel).getByRole("button", { name: "Grade good" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("grade_card", {
        cardId: 101,
        grade: "good",
        durationMs: expect.any(Number),
      });
    });
    await within(panel).findByText("Why narratio before notation?");
    expect(within(panel).getByText(/card 2 of 2/)).toBeDefined();
  });

  it("closes the sitting after the last grade and refreshes the due header", async () => {
    reviewMocks();
    render(<Home />);

    fireEvent.click(await screen.findByRole("button", { name: "Review now" }));
    const panel = await screen.findByLabelText("Review");
    fireEvent.click(await within(panel).findByRole("button", { name: "Reveal" }));
    fireEvent.click(within(panel).getByRole("button", { name: "Grade easy" }));
    await waitFor(() => expect(within(panel).getByText(/card 2 of 2/)).toBeDefined());
    fireEvent.click(within(panel).getByRole("button", { name: "Reveal" }));
    fireEvent.click(within(panel).getByRole("button", { name: "Grade good" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("grade_card", {
        cardId: 102,
        grade: "good",
        durationMs: expect.any(Number),
      });
    });
    await waitFor(() =>
      expect(screen.getByRole("status").textContent).toBe(
        "Sitting done — 2 graded. FSRS scheduled the rest.",
      ),
    );
    expect(await screen.findByRole("button", { name: "Review now" })).toBeDefined();
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_queue");
    });
  });

  it("shows nothing due as a status, not an error", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "get_queue") return Promise.resolve(queueFixture);
      if (cmd === "get_active_session") return Promise.resolve(null);
      if (cmd === "list_due_cards") return Promise.resolve([]);
      return Promise.reject(`unexpected command ${cmd}`);
    });
    render(<Home />);

    fireEvent.click(await screen.findByRole("button", { name: "Review now" }));

    await waitFor(() =>
      expect(screen.getByRole("status").textContent).toBe("Nothing due — reviews are clear."),
    );
    expect(screen.queryByLabelText("Review")).toBeNull();
  });

  it("surfaces queue load failures as an alert", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "get_queue") return Promise.reject("database offline");
      if (cmd === "get_active_session") return Promise.resolve(null);
      return Promise.reject(`unexpected command ${cmd}`);
    });

    render(<Home />);

    await waitFor(() => {
      expect(screen.getByRole("alert").textContent).toBe(
        "Failed to load the queue: database offline",
      );
    });
  });
});
