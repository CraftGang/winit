# Craft suite patches on winit 0.30.13 (`craft/0.30.13`)

Base: upstream tag `v0.30.13` (`e9809ef5`). One commit per patch, original authorship kept. Drop a
patch when a winit release used by the suite contains it. Feature work does not belong here (pen
input lives in `craft-tablet`).

| # | Commit | Change | Upstream | Remove when | Source in the suite |
|---|---|---|---|---|---|
| W1 | `b357a332` | Windows 11: apply the suggested `WM_DPICHANGED` rect (window growth on mixed-DPI drags) | #4341, merged on master | a 0.30.x/0.31 release in use has it | pdfcraft vendor, wordcraft#787, cadcraft#668, photocraft#4007, gridcraft#761, storytold/winit PR #2 |
| W2 | `13ca693b` | Wayland: destroy a seat's text input only when the seat is removed | #4747, merged on master | same | pdfcraft vendor, wordcraft#787, cadcraft#668 |
| W3 | `fd104a4c` | Windows: IME caret from `GCS_CURSORPOS` only when reported (Korean caret) | #4746, open | upstream release | same |
| W4 | `3fffb7ce` | Windows: hidden maximized windows stay hidden until shown | #4587, open (#4575) | upstream release | same |
| W5 | `283370ce` | X11: focus the replacement IME context | #4727, open | upstream release | same |
| W6 | `a82b90fb` | macOS: NSTextInputClient commit flow | #4650 (0.30), open; #4651 merged on master | upstream release | same |
| W7 | `9b90d5bd` | macOS: discard marked text when IME is disabled | #4745, open | upstream release | same |
| W8 | `6a87addd` | macOS: emoji-picker text as `Ime::Commit` (dictation part of #4748 excluded) | #4749 (0.30), open | upstream release | same |
| W9 | `9500f25d` | macOS: no duplicate keyboard text for a Korean IME space | related #4478; to be proposed | upstream fix | same; makes designcraft#1197's workaround unnecessary |
| W10 | `4a7dd791` | macOS: retry the first Apple Korean key | #4744, open | upstream release | same; photocraft#606 |
| W11 | `4df8554b` | X11: scale factor from the server DPI before the panel's physical DPI | none; to be proposed | upstream fix | pdfcraft#1150; photocraft#3724, gridcraft#389, deckcraft#170, wordcraft#721 |
| W12 | `7facf001` | macOS: the view answers `mouseDownCanMoveWindow` from `movable_by_window_background` (title-bar drags reach the app) | none; to be proposed | upstream fix | pdfcraft vendor patch (12), pdfcraft#2347 |
| W13 | `5506b1a9` | Wayland: pixel-only wheel, wheel-tilt and continuous scrolls report `Moved`, so Ctrl zoom doesn't stay latched | none; to be proposed | upstream fix | pdfcraft vendor patch (13), pdfcraft#1569 |
| W14 | `c488d2e9` | macOS: opt-in cancelable termination → close request (+ `tests/macos_termination.rs`); `dpi` from crates.io so git consumers get one `dpi` | none; to be proposed | upstream fix | DesignCraft pin `3d51077f` |
| W15 | `cf73a29c`, `b05ce129` | Wayland: file drops through a per-seat data device; clipboard offered on the same device (`platform::wayland::Clipboard`) | #4729 (0.30), open; master has #4571 (+#4658) | upstream 0.30 release or the egui → 0.31 move | photocraft#7828, filmcraft#1115, pdfcraft#3254; needs egui fork E4 |

Conflict resolved by hand: `wayland/seat/mod.rs` `remove_seat` keeps all three cleanups — cancel a
file drag (W15), remove the seat from winit's clipboard (W15), destroy the seat's text input (W2).

Not in this cut: W16 (Wayland pinch/rotation, #4778), the earlier tablet backport (replaced by `craft-tablet`).
