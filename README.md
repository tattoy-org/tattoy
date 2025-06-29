![Tattoy logo](https://tattoy.sh/assets/screenshots/logo_full.png)

— _logo by [Sam Foster](https://cmang.org)_

# A text-based compositor for modern terminals
![Tattoy with a shader and minmap](https://tattoy.sh/assets/screenshots/hero.webp)

## Usage
See the [documentation](https://tattoy.sh/docs/getting-started/) for full details.

## Design

The engine of Tattoy is a headless terminal emulator called the [Shadow Terminal](https://github.com/tattoy-org/shadow-terminal). Tattoy then takes the purely text-based output of this in-memory terminal, composites it with other text-based layers and then prints it to the user's host terminal. So even though it's a fully-modern terminal, it does not itself manage any GUI windows or font glyph rendering.

### Terminals/Surfaces
There are quite a few terminals, PTYs, shadow PTYs, surfaces, etc, that are all terminal-like in some way, but do different things.

* __The user's actual real terminal__ We don't really have control of this. Or rather, Tattoy as an application merely is a kind of magic trick that reflects the real terminal whilst sprinkling eye-candy onto it. The goal of Tattoy is that you should _always_ be able to recover your original untouched terminal.
* __The PTY (pseudo TTY) of the "original" terminal process__ To achieve the magic trick of Tattoy we manage a "shadow" subprocess of the user's preferred shell, prompt, etc. It is managed completely in memory and is rendered headlessly. The PTY code itself is provided by the [portable_pty](https://docs.rs/portable-pty/latest/portable_pty/) crate from the [Wezterm project](https://github.com/wez/wezterm) ❤️.
* __The headless rendering of the user's "original" terminal__ This is just a headless rendering of the underlying shadow PTY by an in-memory terminal emulator. This is done with a the [Shadow Terminal](https://github.com/tattoy-org/shadow-terminal) which in term depends on [wezterm_term::Terminal](https://github.com/wez/wezterm/blob/main/term/README.md) ❤️.
* __The composited Tattoy "surface"__ A surface here refers to a [termwiz::surface::Surface](https://github.com/wez/wezterm/tree/main/termwiz). It represents a terminal screen but is not an actual real terminal, it's merely a structured representation. This is where we can create all the magical Tattoy eye-candy. Although it does not intefere with the shadow TTY, it can be informed by it. Hence why you can create Tattoys that seem to interact with the real terminal. In the end, this Tattoy surface is composited with the contents of the shadow PTY.
* __The shadow terminal "surface"__ This is merely a copy of the current _visual_ status of the shadow terminal. We don't use the actual shadow terminal emulator as the source because it's possible that this data is queried frequently by various Tattoys. Querying the cached visual representation is more efficient than querying an actual TTY, even if it exists only in memory.
* __The final composite "surface"__ This is the final composited surface of the both the underlying shadow terminal and all the active Tattoys. A diff of this with the user's current real terminal is then used to do the final update to the user's live terminal.

## Testing
We use [`cargo nextest`](https://nexte.st), which you will need to install if you haven't already.
```
cargo build --all; cargo nextest run
```

In CI I use `cargo nextest run --retries 2` because some of the e2e tests are flakey.

## Debugging
* Set `log_level = "trace"` in `$XDG_CONFIG_DIR/tattoy/tattoy.toml`
* Default log path is `$XDG_STATE_DIR/tattoy/tattoy.log`.
* Log path can be changed with `log_path = "/tmp/tattoy.log"` in `$XDG_CONFIG_DIR/tattoy/tattoy.toml`
* Or log path can be changed per-instance with the `--log-path` CLI argument.
