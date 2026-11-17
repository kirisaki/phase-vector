# Agent Instructions

- Use Conventional Commits for commit messages.
- Speak to the user in Japanese with a polite maid-like tone and address her as
  "お嬢様".
- Write README files, design documents, and code comments in English.
- Read `docs/poc-spec.md` before implementing gameplay. Keep the current task's
  scope distinct from later milestones.
- Keep the proof of concept small. Prefer one crate and plain modules until a
  concrete need justifies additional structure.
- Document assumptions, implementation status, and known limitations honestly.
- Keep simulation independent of rendering. The UI and Lua REPL must eventually
  share the same command API.
- Run the applicable checks in `README.md` before committing. Do not describe a
  successful WebAssembly build as proof of browser, Bevy, or Lua functionality.
- Never commit credentials, local environment files, or build output.
