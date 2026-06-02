# ankra
A flexible, table-based input method engine (IME) built specifically for Unix systems running Wayland.

## motivation
I wanted a way to input Chinese characters natively on Wayland. Not satisfied with the ambiguity of Pinyin, I looked into structural, table-based IMEs (such as Cangjie) and was impressed by their speed and precision. Sadly, robust support for table-based input methods on Wayland is still lacking. Rather than compromising, I decided to build a lean, native tool from the ground up.

## architecture

Ankra is split into 4 internal crates.

* **`libankra` (Engine Core):** A stateless, pure-Rust implementation of a table-driven input processor. It handles dictionary loading, candidate matching grids, weighting, boundary guards, and text state responses (`Commit`, `Suggest`, `Empty`). It has zero awareness of Wayland, window systems, or I/O loops.
* **`ankra-wayland` (Protocol Orchestrator):** The system middleware that handles low-level interaction with the display server. It translates raw compositor protocol frames into unified events for the underlying engine.
* **`ankrad` (System Daemon):** Spawns the Wayland event loops, manages configuration paths, handles live toggles, and tracks asynchronous operational threads.
* **`ankra-cli` (Controller):** A lightweight command-line utility used to communicate with the running `ankrad` daemon. It allows the user to dynamically toggle the engine state, switch layouts on the fly, or poll daemon status, making it easy to integrate into status bars or global window manager keybindings.

## design philosophy

Ankra bridges the gap between traditional static muscle memory (strict Wubi/Cangjie) and adaptive prediction. It achieves this through a few core UX rules:
- *The Exact-Match*: Touch-typing muscle memory is mostly protected, as exact sequence matches always take priority over heavier predictive phrases. If you memorize that hq = 牛, a heavily used phrase like hqhpi (我的) will never hijack your keystroke.
- *Adaptive Frequency Weighting*: The engine actively learns your vocabulary. If two characters share the exact same sequence (i.e. a collision), the candidate you commit gets heavier and bubbles to the top.
- *Self-Stabilizing Personalisation*: Ankra uses absolute historical frequency rather than chaotic, constantly shifting context-prediction. The engine starts out raw, but quickly carves a permanent "groove" optimized exactly for what you write. It becomes a hyper-personalised static layout.
- *Phrases as Optional Speed Boosts*: You can type full phrases blindfolded using their exact radical codes, or you can glance at the screen and hit Space early when the engine accurately predicts your most heavily used words.

## installation

### from source

```
cargo install --path src/ankrad
```

## configuration
As of now all the configuration is done in `$XDG_CONFIG_HOME/ankra`, where a single layout will have it's own folder consisting of the following 2 files:
- `table.csv`: a simple csv structued as `character,sequence`, where `character` is the ouput, and `sequence` is the key inputs required.
- `config.zm`, which is composed of two structures:
	- `keys`: associates a character to a keycode, said character will be used for lookup in the table.
	- `specs`: associates a function to a keycode, an exhaustive list of functions may be found in the example config.
- `phrases.csv`: an optional csv that appends to table.csv, consisting of multi-character outputs.

## ideas

- button to show several candidates at a time?
