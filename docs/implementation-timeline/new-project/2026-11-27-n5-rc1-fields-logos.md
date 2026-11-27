# Phase 15 N5: rc.1 and the owner's changes to the New project sheet

> Date: 2026-11-27 · Branch: `phase15-ui` · Plan and results: `docs/roadmap/phase-15.md`, "rc.1" · Decision: D-063

## What changed

- **`crates/gui/ui/app.js`:**
  - **`npSelect`:** the sheet's select field, which is the app's dropdown in the Name input's shape. Its list opens under it, as wide as the field, and an option may be disabled with a title.
  - **The form:** Name and Folder are full width, and PHP, Starter kit or Kind, and Database are selects in one row (`np-opts`), in place of the segmented controls. A disabled starter kit's reason shows under the row.
  - **`NP_LOGO` and `npLogo`:** each template's brand mark from Simple Icons 16.37.0 (CC0-1.0), in its brand color, or the text color for the black marks. They replace the initials and tile colors in `NP_TPL`.
- **The list's scroll** (`npDraw`): a redraw keeps the template list's scroll, and the form's while its template stays. The chosen template is scrolled into view. Before, picking Express near the list's end sent the list back to the top (the owner's report).
- **The focus** (`npDraw`, `npSelect`): a pick returns the focus to the template list or to the select, never to the Name field. The field takes it when the sheet opens only, and turns off macOS's writing suggestions (the owner's report: its suggestions opened after every pick).
- **The folder:** with no parked folder, the Folder field opens the folder chooser directly (the owner's request). `crates/gui/src/api.rs`: a new project's chooser starts at the home folder.
- **`crates/gui/ui/app.css`:** `np-field` (28 px, one border, radius, fill and type for the input and the selects), `np-opts`, and `np-logo` (a neutral tile), in place of `np-input`, `np-two` and `np-tile`.
- **`README.md`:** where the logos come from, and that they are their owners' trademarks.
- **The docs:** rc.1's results on the M1 and the VMs, the owner's two requests, and the change (`phase-15.md`, D-063).

## Checked

- **In the browser harness,** in both themes:
  - the five fields' size, border and fill;
  - the Folder list's position and width;
  - the fields of each template;
  - the disabled kits with Node 20;
  - a Laravel project with MySQL and the React kit made through the selects;
  - Express picked near the list's end: N4's code sent the list to the top, and the fix keeps it, with ↑ ↓ wraps in view and the form's place kept for a database pick;
  - the focus after a template pick and a select's pick, and the Folder field's chooser with and without a parked folder.
- `osacompile` accepts `choose_folder`'s new AppleScript. `cargo clippy` and `cargo test` pass for `devid-gui`.
- `node --check` passes on `app.js`.
