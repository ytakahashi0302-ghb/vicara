# vicara

<p align="center">
  <img src="./public/logos/banner.png" alt="vicara banner" width="800" />
</p>

<p align="center">
  <strong>Human-led AI team orchestration for solo builders.</strong>
</p>

<p align="center">
  <a href="./README.md">🇯🇵 日本語</a>
</p>

---

## What is vicara?

**vicara** is a local-first desktop app that lets a solo developer lead multiple AIs as a "team", putting deliberation and decision-making at the center while accelerating implementation and verification.

vicara is not a tool that delegates everything to AI.
The human acts as the **Product Owner** — deciding *what to build* and *in what order* — while AIs follow those decisions to accelerate implementation, research, and verification. This relationship is naturally managed through the common language of **Scrum**.

> Start your first step without hesitation, lead multiple AIs as a team, stay in control without being swallowed by a black box, and move forward on the right path.

vicara integrates idea brainstorming, project context organization, sprint planning, role assignment, and implementation execution via coding agent CLIs into a single UI.

![vicara overview](./docs/images/vicara-overview-v2_0_0.png)

---

## Key Features

| Feature | Description |
|---------|-------------|
| **PO Assistant** | Sidebar AI that supports Product Owner decisions — priority sorting, requirements clarification, progress judgment |
| **Dev Agent** | Implementation AI that executes tasks via coding agent CLIs (Claude Code / Gemini / Codex) based on role templates |
| **AI Retrospective** | Post-sprint reflections using the KPT (Keep, Problem, Try) framework. SM/PO agents auto-extract insights and synthesize summaries |
| **Improvement Loop** | Convert approved "Try" items into project `Rule.md` automatically to continuously refine AI team behavior |
| **Inception Deck** | Build `PRODUCT_CONTEXT.md`, `ARCHITECTURE.md`, `Rule.md` through AI-driven brainstorming |
| **Project Notes** | Persistent note-taking for project-level context shared across all AI agents |
| **Scaffold** | Tech stack detection, initial directory setup, `AGENTS.md` / `.claude/settings.json` generation |
| **AI Task Decomposition** | Break down PBIs (Product Backlog Items) into actionable task granularity |
| **Interactive Kanban** | Visual management of PBIs and tasks with project-based serial numbering (e.g., PBI-1, Task-5) |
| **Terminal Dock** | VS Code-like tabbed terminal with improved real-time output streaming for AI agents |
| **Multi-Agent Execution** | Launch coding agent CLIs per role, implementing tasks in parallel |
| **Git Worktree Review** | Isolated environments per task, with preview, approve-merge, and conflict resolution in one flow |
| **LLM Observability** | Visualize token usage and estimated costs per project / sprint |
| **Resizable 3-Pane UI** | Adjust Kanban / Terminal / PO Assistant layout to your workflow |
| **Local-First** | Safe and transparent operation built on local directories and local DB |

---

## Getting Started

### Prerequisites

- [Node.js](https://nodejs.org/) (LTS recommended)
- [Rust](https://www.rust-lang.org/tools/install) / Cargo
- At least one coding agent CLI: [Claude Code](https://docs.anthropic.com/en/docs/claude-code) / [Gemini CLI](https://github.com/google-gemini/gemini-cli) / [Codex CLI](https://github.com/openai/codex)

### Installation & Launch

```bash
git clone https://github.com/ytakahashi0302-ghb/vicara.git
cd vicara
npm install
npm run tauri -- dev
```

### LLM Setup

vicara supports multiple LLM providers — Claude API, Gemini API, OpenAI API, and Ollama.
For detailed setup instructions, see:

👉 **[LLM Setup Guide](./docs/llm-setup.md)** | [日本語版](./docs/llm-setup_ja.md)

### Set Working Directory

From the project area on the left side of the header, select a workspace and set the target directory with the folder button.
This local path becomes the working directory for each Dev Agent.

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Frontend | React 19, TypeScript, Tailwind CSS v4 |
| Backend | Tauri v2 (Rust) |
| Database | SQLite (local) |
| State Management | React Context / Hooks |
| AI | Claude Code CLI, Gemini CLI, Codex CLI, Anthropic API, Gemini API, OpenAI API, Ollama |
| Terminal | xterm.js |
| UI Icons | Lucide React |

---

## Development

```bash
npm run dev             # Vite dev server
npm run build           # Frontend build
npm run lint            # ESLint
npm run tauri -- dev    # Launch Tauri app (dev)
npm run tauri -- build  # Production build
```

For design guidelines and development rules, see [Rule.md](./Rule.md).
For architecture overview, see [ARCHITECTURE.md](./ARCHITECTURE.md).

---

## Origin of the Name

The name has two origins:

1. **Bikara (毘羯羅)** — one of the Twelve Heavenly Generals in Buddhist mythology, symbolizing the beginning (the Rat in the Chinese zodiac) and radiating "universal illumination" to guide the world toward the right start.
2. **Vicāra** — a Sanskrit/English word meaning "thought, deliberation, investigation, planning."

vicara is designed at the intersection of these two meanings.

---

## License

This project is licensed under the [Apache License 2.0](./LICENSE).

---

## Release Notes

Latest release:
- [vicara v2.2.0](./releases/v2.2.0.md)

Looking for localized versions?
- [🇯🇵 日本語 (Main)](./README.md)
- [🇺🇸 English (Current)](./README_en.md)

---

*vicara v2.2.0 — Human-led AI team orchestration for solo builders.*
