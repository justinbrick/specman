# About SpecMan

SpecMan exists because existing "AI spec assistants" such as GitHub's Speckit lean into corporate optics, ambiguous prompting, and vague approvals instead of helping engineers capture executable behaviors. Speckit's defaults reward paperwork for shareholders; SpecMan rejects that pattern and focuses on giving programmers a lean system for writing, validating, and iterating on specifications without begging for bureaucratic permission.

## What Problem Are We Solving?
- **Poor prompting and context loss:** Speckit-style flows bury requirements inside mushy prompts. SpecMan enforces explicit Markdown structures, RFC&nbsp;2119 keywords, and deterministic templates so intent survives copy/paste and AI passes.
- **Company-coloring bias:** Many vendor tools assume top-down approval chains. SpecMan centers the people implementing the system, keeping artifacts in-repo, reviewable, and versioned like code.
- **Behavior drift:** Specs that read like memos never map to code. SpecMan couples every document to a concrete data model and dependency graph so changes ripple predictably across specs, implementations, and scratch pads.

## How SpecMan Responds
1. **Data-first contracts:** The [SpecMan Data Model](../spec/specman-data-model/spec.md) defines exact YAML schemas for workspaces, specs, implementations, and scratch pads. Every downstream tool validates against these contracts, ending format guesswork.
2. **Deterministic platform services:** [SpecMan Core](../spec/specman-core/spec.md) delivers workspace discovery, dependency mapping, lifecycle automation, and metadata mutation as reusable behaviors instead of ad-hoc scripts.
3. **Template & prompt governance:** The [SpecMan Templates](../spec/specman-templates/spec.md) catalog ships Markdown scaffolds and AI prompts that preserve HTML comment guardrails. Automation must satisfy each directive before removing it.
4. **Operator-focused CLI:** The [SpecMan CLI](../spec/specman-cli/spec.md) and its Rust implementation expose declarative commands (`spec`, `impl`, `scratch`, `status`) so engineers can create or delete artifacts without silent side effects.

## Philosophy
- **Behavior over bureaucracy:** We optimize for defining what the system must do. Anything about shareholder rituals or executive sign-offs belongs elsewhere.
- **Workspace sovereignty:** Everything lives in your repository, inside a `.specman` root. There is no dependency on remote approval queues or mystery SaaS state.
- **Deterministic automation:** Commands, templates, and dependency graphs must remain scriptable and diffable. If two runs have the same inputs, the outputs must match byte-for-byte.
- **AI as a power tool, not a gatekeeper:** Prompt templates are defensive, explicit, and auditable. They are there to accelerate authoring, not to hide requirements inside proprietary chats.

## Core Outcomes
- **Faster onboarding:** New contributors can read consistent specs and implementations without reverse engineering an organization's folklore.
- **Safer change impact:** Dependency graphs across specs, implementations, and scratch pads highlight downstream blast radius before a file is edited.
- **Template-aligned artifacts:** Markdown scaffolds ensure every document ships with required metadata, terminology sections, and unique headings.
- **No wasted cycles:** Engineers spend time shaping behaviors and code—not navigating compliance theater.

## Learn More
- [SpecMan Data Model](../spec/specman-data-model/spec.md)
- [SpecMan Core](../spec/specman-core/spec.md)
- [SpecMan Templates](../spec/specman-templates/spec.md)
- [SpecMan CLI](../spec/specman-cli/spec.md)
- [Markdown Templates Implementation](../impl/markdown-templates/impl.md)
- [SpecMan Library (Rust)](../impl/specman-library/impl.md)
- [SpecMan CLI (Rust)](../impl/specman-cli-rust/impl.md)
