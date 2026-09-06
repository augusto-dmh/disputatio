#!/usr/bin/env python3
"""fitness.py — governance-as-code for disputatio (ADR 0008).

Architecture boundaries and repo contracts, enforced in `make lint` and CI.
Principle: "enforçar no código, não só no discurso" (TLC fundamentos, classes 21/39/62).
Rules light up as the repo grows; checks that don't apply yet are skipped, not broken.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

ERRORS: list[str] = []
WARNINGS: list[str] = []


def error(msg: str) -> None:
    ERRORS.append(msg)


def warn(msg: str) -> None:
    WARNINGS.append(msg)


def check_adr_naming() -> None:
    adr_dir = ROOT / "docs" / "adr"
    if not adr_dir.is_dir():
        return
    pattern = re.compile(r"^(\d{4})-[a-z0-9-]+\.md$")
    seen: dict[int, str] = {}
    for f in sorted(adr_dir.glob("*.md")):
        m = pattern.match(f.name)
        if not m:
            error(f"adr filename does not match 'NNNN-slug.md': docs/adr/{f.name}")
            continue
        num = int(m.group(1))
        if num in seen:
            error(f"duplicate ADR number {num:04d}: {seen[num]} and {f.name}")
        seen[num] = f.name
    if seen:
        gaps = sorted(set(range(1, max(seen) + 1)) - set(seen))
        for g in gaps:
            warn(f"ADR gap: {g:04d} missing (fine if intentional)")


def check_agents_md() -> None:
    agents = ROOT / "AGENTS.md"
    if not agents.exists():
        error("AGENTS.md missing — the always-on context is the harness core")
        return
    lines = agents.read_text(encoding="utf-8").splitlines()
    if len(lines) > 160:
        error(f"AGENTS.md has {len(lines)} lines (cap 160) — move depth to skills or docs")


def check_specs_cap() -> None:
    specs = ROOT / "specs"
    if not specs.is_dir():
        return
    allowed = {"requirements.md", "design.md", "tasks.md"}
    for feature in sorted(p for p in specs.iterdir() if p.is_dir()):
        if feature.name == "_templates":
            continue
        extra = [f.name for f in feature.iterdir() if f.is_file() and f.name not in allowed]
        if extra:
            error(f"specs/{feature.name}/ exceeds the 3-file cap: {', '.join(extra)}")


def check_domain_purity() -> None:
    domain = ROOT / "src-tauri" / "src" / "domain"
    if not domain.is_dir():
        return
    banned = ["tauri::", "sqlx", "rusqlite", "reqwest", "tokio_postgres", "diesel", "serde_json::from_str"]
    for rs in domain.rglob("*.rs"):
        text = rs.read_text(encoding="utf-8", errors="replace")
        for token in banned:
            if token in text:
                error(f"domain imports infrastructure: src-tauri/src/domain/{rs.relative_to(domain)} contains '{token}'")


def check_ts_boundary() -> None:
    src = ROOT / "src"
    if not src.is_dir() or not (ROOT / "src-tauri").is_dir():
        return
    for ts in list(src.rglob("*.ts")) + list(src.rglob("*.tsx")):
        text = ts.read_text(encoding="utf-8", errors="replace")
        if "../src-tauri" in text or "/src-tauri/" in text:
            error(f"TS imports Rust paths (use invoke): {ts.relative_to(ROOT)}")


def check_compose() -> None:
    compose = ROOT / "docker" / "docker-compose.yml"
    if not compose.exists():
        error("docker/docker-compose.yml missing — the state store contract (ADR 0003)")
        return
    text = compose.read_text(encoding="utf-8")
    if "disputatio_pgdata" not in text:
        error("compose lost the named volume 'disputatio_pgdata' (ADR 0003)")
    if "5433" not in text:
        error("compose lost host port 5433 (collides with other local Postgres)")


def main() -> int:
    check_adr_naming()
    check_agents_md()
    check_specs_cap()
    check_domain_purity()
    check_ts_boundary()
    check_compose()

    for w in WARNINGS:
        print(f"WARN: {w}")
    for e in ERRORS:
        print(f"ERROR: {e}")
    print(f"fitness: {len(ERRORS)} error(s), {len(WARNINGS)} warning(s)")
    return 1 if ERRORS else 0


if __name__ == "__main__":
    sys.exit(main())
