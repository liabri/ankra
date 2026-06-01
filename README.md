# ankra
A flexible, table-based input method engine (IME) built specifically for Unix systems running Wayland.

## motivation
I wanted a way to input Chinese characters natively on Wayland. Not satisfied with the acoustic ambiguity of Pinyin, I looked into structural, table-based IMEs (such as Cangjie) and was thoroughly impressed by their speed and precision. Sadly, robust support for table-based input methods on Wayland is still severely lacking. Rather than compromising with heavy legacy wrappers, I decided to build a lean, native tool from the ground up.

## architecture

Ankra is split into decoupled crates following the principle of clean separation between core state mechanics and environment orchestration:

* **`libankra` (Engine Core):** A stateless, pure-Rust implementation of a table-driven input processor. It handles dictionary loading, candidate matching grids, boundary guards, and text state responses (`Commit`, `Suggest`, `Empty`). It has zero awareness of Wayland, window systems, or I/O loops.
* **`ankra-wayland` (Protocol Orchestrator):** The system middleware that handles low-level interaction with the display server. It translates raw compositor protocol frames into unified events for the underlying engine.
* **`ankrad` (System Daemon):** The entry-point binary. It spawns the Wayland event loops, manages configuration paths, handles live toggles, and tracks asynchronous operational threads.
* **`ankra-cli` (Controller):** A lightweight command-line utility used to communicate with the running `ankrad` daemon. It allows the user to dynamically toggle the engine state, switch layouts on the fly, or poll daemon status, making it easy to integrate into status bars or global window manager keybindings.

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




## plans

- Phrase support and multi character (from a CSV or guessing) eg. HIDP => 我想 (HQI DUP)
- Personal dictionary (which will alter weights)
