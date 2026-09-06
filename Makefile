# Verification vocabulary — same names for human, agent, and CI (docs/adr/0008)
.DEFAULT_GOAL := check
COMPOSE := docker compose -f docker/docker-compose.yml --env-file .env

.PHONY: setup infra down dev lint test check fitness eval

setup: ## one-time: wire git hooks (conventional commits)
	git config core.hooksPath .githooks
	@echo "hooks wired: .githooks/commit-msg"

infra: ## start Postgres (ADR 0003; needs Docker daemon)
	@test -f .env || (cp .env.example .env && echo "created .env — set POSTGRES_PASSWORD if you care to")
	$(COMPOSE) up -d --wait
	$(COMPOSE) ps

down: ## stop Postgres (volume persists)
	$(COMPOSE) down

dev: ## run the app
	@echo "not yet — arrives with slice 0.1 (tauri dev)"

lint: fitness ## fitness functions + linters (linters arrive with slice 0.1)

fitness: ## governance-as-code: architecture boundaries (ADR 0008)
	python3 scripts/fitness.py

test: ## unit/integration tests
	@echo "not yet — vitest/cargo test arrive with slice 0.1"

check: lint test ## THE GATE — run before every PR

eval: ## AI-output evals
	@echo "not yet — cases land with the first prompts (slice 0.3); see evals/README.md"
