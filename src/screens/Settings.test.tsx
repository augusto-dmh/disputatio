import { invoke } from "@tauri-apps/api/core";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import Settings from "./Settings";

// The screen talks to Rust only through invoke (AGENTS.md boundary) — via the
// generated tauri-specta bindings, which call it under the hood.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

// Testing Library's auto-cleanup needs global afterEach; vitest globals are off.
afterEach(cleanup);

beforeEach(() => {
  invokeMock.mockReset();
});

describe("Settings screen", () => {
  it("loads and renders the stored settings through invoke", async () => {
    invokeMock.mockResolvedValue({
      vault_path: "/home/augusto/vault",
      track_positions: {
        "system-design": "courses/system-design/classes/class-004.md",
      },
    });

    render(<Settings />);

    await waitFor(() => {
      const vault = screen.getByLabelText("Vault path") as HTMLInputElement;
      expect(vault.value).toBe("/home/augusto/vault");
    });
    const position = screen.getByLabelText("system-design") as HTMLInputElement;
    expect(position.value).toBe("courses/system-design/classes/class-004.md");
    expect(invokeMock).toHaveBeenCalledWith("get_settings");
  });

  it("saves the vault path and a position override via set_settings", async () => {
    invokeMock.mockResolvedValue({ vault_path: "/vault", track_positions: {} });

    render(<Settings />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("get_settings"));

    fireEvent.change(screen.getByLabelText("Vault path"), {
      target: { value: "/vault" },
    });
    fireEvent.change(screen.getByLabelText("fundamentos-enterprise"), {
      target: { value: "courses/fundamentos-enterprise/classes/class-005.md" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenLastCalledWith("set_settings", {
        settings: {
          vault_path: "/vault",
          track_positions: {
            "fundamentos-enterprise": "courses/fundamentos-enterprise/classes/class-005.md",
          },
        },
      });
    });
    await waitFor(() => expect(screen.getByRole("status").textContent).toBe("Settings saved."));
  });

  it("sends null for an empty vault path and lets blank overrides clear", async () => {
    invokeMock.mockResolvedValue({
      vault_path: "/stored",
      track_positions: { videos: "videos/some-video.mp4" },
    });

    render(<Settings />);
    await waitFor(() => {
      const videos = screen.getByLabelText("videos") as HTMLInputElement;
      expect(videos.value).toBe("videos/some-video.mp4");
    });

    fireEvent.change(screen.getByLabelText("Vault path"), { target: { value: "" } });
    fireEvent.change(screen.getByLabelText("videos"), { target: { value: "" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenLastCalledWith("set_settings", {
        settings: {
          vault_path: null,
          track_positions: { videos: "" },
        },
      });
    });
  });

  it("surfaces save failures in the status line", async () => {
    invokeMock.mockResolvedValue({ vault_path: null, track_positions: {} });

    render(<Settings />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("get_settings"));

    invokeMock.mockRejectedValueOnce("settings store error: boom");
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(screen.getByRole("status").textContent).toBe(
        "Failed to save settings: settings store error: boom",
      ),
    );
  });
});
