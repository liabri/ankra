# benten (WIP)
A flexible input method engine made and tested only for unix systems running wayland, which allows the easy configuration of mixed-script writing systems such as Kana/Kanji & Hangul/Hanja.

## motivation
As I started to learn Japanese, I wanted a way to input without having to rely on candidate windows and such, which led me to find Cangjie and other shaped based input method for chinese characters. This idea was to utilise a kana layout with cangjie hidden away behind a single key press, this could've easily been done in Rime or just using a keyboard remapper, but I thought it'd be a fun project and would satisfy my desire of not jerry rigging appplications together. This has led to the birth of `benten`, a decently opinionated but flexible input method engine.

## why the name `benten` ?
Named after the Japanese Buddhist Goddess "Benzaiten" (弁才天) whom stands for all things that flow (as per Wikipedia), which hopefully represents this project well :).  

## todo
- Compose based on surrounding text, eg: "+" then a "-" would replace them with a "±"
- Unicode method;
- [Glyph variant forms](https://en.wikipedia.org/wiki/Variant_form_(Unicode))
- Possibly abstract key codes;
- Prevent recreation of `BaseDirectories` struct in deserialisation methods in global parser;
- BTreeMaps/IndexMap/AHash ?

## configuration
As of now all the configuration is done in `$XDG_CONFIG_HOME`, consisting of two folders: 
1. `layouts`: key map and layout configuration, defined in `*.layout.zm`;
2. `tables`: tables for table-lookup (still requires a layout file), defined in `*.dict.zm`;