# Pulp: guidance for AI coding agents

Pulp is a free, open-source, community-developed writing app and project organiser for writers, students and creatives. Projects are ordinary folders of plain, readable files, so people's work outlives the app. It works fully offline and never loses work. Pulp itself has no AI features, in any version.

## Rules

- Read `docs/specs/pulp-master-design.md` and the current milestone spec in `docs/specs/` before changing behaviour.
- `pulp-core` stays UI-free and does all I/O through `Storage`.
- Use test-driven development: write the failing test first and watch it fail.
- Commit with `git commit -s`.
- Never weaken a test to make it pass.
- Explain results to Gonçalo in plain language. Gonçalo does not read code.
