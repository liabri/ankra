# ankra
A flexible table based input method engine made and tested only for unix systems running wayland

## motivation
I wanted a way to input chinese characters, not satisfied with pinyin I looked at table based IMEs and was very much impressed. Sadly there is not much support for them on wayland for now, so I decided to make my own!

## plans

- Phrase support (from a CSV or guessing) eg. HIDP => 我想 (HQI DUP)
- On enter, input current key_sequence instead of doing nothing, enabling the input of latin characters (albeit annoyingly)
- Multiple character support, guessing how to separate them (i.e. not having to press space between every character), similar to phrase support but not quite.
- Potentially show key_sequence in inline suggestion instead of just character
- Personal dictionary (which will alter weights)

## installation

### from source

```
cargo install --path src/ankrad
```

## configuration
As of now all the configuration is done in `$XDG_CONFIG_HOME/ankra`, where a single layout will have it's own folder consisting of the following 2 files:
- `table.csv`
- `config.zm`, which is composed of two structures:
	- `keys` Associates a character to a keycode, said character will be used for lookup in the table.
	- `specs` Associates a function to a keycode, an exhaustive list of functions may be found in the example config.
