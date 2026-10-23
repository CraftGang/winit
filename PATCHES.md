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
| W17 | `2be27382` | Regression tests for W2, W3, W5–W10, W12, W13: source guards (`tests/ime_patches.rs`, `tests/pointer_patches.rs`) and the W13 scroll-phase unit tests | n/a (tests) | each guard goes with its patch | storytold/winit PR #3; #4 (pointer tests) |
| W18 | `894bf2d7` | Windows: a focused window that allows IME keeps a hidden system caret over the IME cursor area (Sogou, some Japanese IMEs place their windows at the caret) | none; to be proposed | upstream fix | storytold/winit PR #4; craft-libs#198 (I6) |
| W19 | `fa2e1b3d` | Windows: a key-down the IME takes (`VK_PROCESSKEY`) isn't sent as a key press unless the IME passes text on with it | none; to be proposed | upstream fix | storytold/winit PR #4; craft-libs#198 (I6) |
| W20 | `019c95f1` | X11: one input context per window; disallowing IME resets and unfocuses it, so input methods keep their state (Vietnamese Telex). Replaces W5's refocus | none (master still recreates the context); to be proposed | upstream fix (drop with W5) | storytold/winit PR #5; craft-libs#198 (I3) |
| W21 | `23bb19a7` | X11: send the IME spot once the input method has been quiet for 100 ms, so keys it forwarded aren't filtered again (garbled Vietnamese) | issue #4777; to be proposed | upstream fix | storytold/winit PR #5; wordcraft#2932 |
| W22 | `0816871e` | macOS: the view reports a caret (`ImeDocument`) and honours replacement ranges next to it (press-and-hold accents, dictation); text typed without a composition is committed (`，`, ideographic space); input methods calling a view without a window don't abort | teardown guard is master's d6f7a28 (#4333); #3617, #4666 open | upstream release | storytold/winit PR #6; craft-libs#198 (I5) |
| W23 | `c8cd8aa1` | macOS: keys pressed with ⌘ are named by the layout's ⌘ table (`KeyEventExtMacOS::key_with_command`); ⌘V on "Dvorak – QWERTY ⌘" | none; to be proposed | upstream fix | storytold/winit PR #6; pdfcraft#3655, craft-libs#198 (K1) |
| W24 | `4b5bc5a0` | `Window::reset_ime()`: discard the composition without committing (X11 `XmbResetIC`, Wayland disable/enable, Windows `CPS_CANCEL`, macOS `discardMarkedText`) | none; to be proposed | upstream release with an equivalent | storytold/winit PRs #5, #6; craft-libs#198 (I4) |
| W25 | `eb4007da` | X11/Wayland: `platform::latin_layout::KeyEventExtLatinLayout`, the key on the first Latin layout group for shortcuts on non-Latin layouts | none; to be proposed | upstream release with an equivalent | storytold/winit PR #5; craft-libs#198 (K2) |
| W26 | `2b9cd1cf` | macOS: replaced text is deleted by UTF-8 byte count with the commit (`WindowExtMacOS::take_ime_delete_surrounding`, same shape as W27) | master: `Ime::DeleteSurrounding` (a new `Ime` variant breaks 0.30 matchers) | the egui → 0.31 move | storytold/winit PR #6 (`d103edae`) |
| W27 | `f40c76a2` | Wayland: text-input-v3 surrounding text (`WindowExtWayland::set_ime_surrounding_text`) and delete-surrounding attached to the commit (`take_ime_delete_surrounding` → `ImeDeleteSurrounding { before_bytes, after_bytes }`) | master has both as core API (`Ime::DeleteSurrounding`) | the egui → 0.31 move | storytold/winit PR #7; craft-libs#198 (I7) |

Conflict resolved by hand: `wayland/seat/mod.rs` `remove_seat` keeps all three cleanups — cancel a
file drag (W15), remove the seat from winit's clipboard (W15), destroy the seat's text input (W2).

W17–W27 are the language-support patches (storytold/craft-libs#198); W16 stays with the pinch/rotation
patch below.

W22 gives every macOS window with IME allowed a real caret, so input methods send replacement
ranges: press-and-hold accents, dictation corrections, and input methods that switch from marked
text to `insertText:replacementRange:` once they see a caret. An app must call
`WindowExtMacOS::take_ime_delete_surrounding` (W26) when it handles each `Ime::Commit`, or the
replaced text stays and the new text is typed after it. egui-winit `craft/0.37` calls it. With the
caret, a held letter in such a window also stops repeating, because AppKit's press-and-hold takes the
repeats, as in native text fields.

Not in this cut: W16 (Wayland pinch/rotation, #4778), the earlier tablet backport (replaced by `craft-tablet`).
