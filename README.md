DWFV
====

[![Build Status](https://travis-ci.com/psurply/dwfv.svg?branch=master)](https://travis-ci.com/psurply/dwfv)
[![Crates.io](https://img.shields.io/crates/v/dwfv)](https://crates.io/crates/dwfv)
[![Docs Status](https://docs.rs/dwfv/badge.svg)](https://docs.rs/crate/dwfv/)

A simple digital waveform viewer with vi-like key bindings.

```shell
$ dwfv sample.vcd
```

![screenshot](docs/screenshot.png)

The tool takes a Value Change Dump (VCD) file (as defined by IEEE Standard
1364-1995) as input and displays the waveforms using
[tui-rs](https://github.com/fdehau/tui-rs).

The backend API which facilitates the manipulation of digital signals in Rust
is also exposed and can be used independently of the TUI.

Installation
------------

```shell
$ cargo install dwfv
```

From sources:

```shell
$ cargo install --path .
```

Key Bindings
------------

### Global

- `q`: quit

### Cursor movement

- `h`/Left: move cursor left
- `j`/Down: move cursor down
- `k`/Up: move cursor up
- `l`/Right: move cursor right
- `w`: jump forward to the next rising edge
- `e`: jump forward to the next falling edge
- `b`: jump backward to the previous rising edge
- `0`: jump to timestamp 0
- `^`/Home: jump to the first event
- `$`/End: jump to the last event
- `gg`: jump to first signal
- `G`: jump to last signal

### Frame

- `zi`/`+`: zoom in
- `zo`/`-`: zoom out
- `zc`/`=`: zoom fit
- `zz`: center cursor on screen

### Editing

- `o`: edit layout
- `dd`/Delete: delete the selected signal
- `yy`: copy the selected signal
- `p`: paste the clipboard after cursor
- `P`: paste the clipboard before cursor
- `u`: undo
- `r`: redo
- `c`: show clipboard

### Search

- `f`: search for event in the selected signal
- `/`: search for pattern in the signal's names
- `n`: repeat search forward
- `N`: repeat search backward

### Visual mode

- `v`: start visual mode
- `<enter>`: zoom fit the selected time frame

### Mouse

- Left click: move cursor
- Right click: zoom out
- Wheel up: zoom in
- Wheel down: zoom out
- Hold/release left click: zoom fit the selected time frame

Command-Line Interface
----------------------

### Show some stats about the VCD file

```shell
$ dwfv examples/sample.vcd --stats
test
  ! (value) - width: 8, edges: 37, from: 0s, to: 1010s
  " (clk) - width: 1, edges: 102, from: 0s, to: 1010s
  # (reset) - width: 1, edges: 5, from: 0s, to: 620s
  c1
    " (clk) - width: 1, edges: 102, from: 0s, to: 1010s
    # (reset) - width: 1, edges: 5, from: 0s, to: 620s
    $ (out) - width: 8, edges: 37, from: 0s, to: 1010s
```

### Display values of the signals at a given time

```shell
$ dwfv sample.vcd --at 1337
test
  ! (value) = h14
  " (clk) -> h1
  # (reset) = h0
  c1
    " (clk) -> h1
    # (reset) = h0
    $ (out) = h14
```

### Search in the waveforms

Events in the waveforms can be searched using the '--when' option. Examples:

- Searching when the `value` signal is equal to `2`:

```shell
$ dwfv sample.vcd --when '$! = 2'
310s-330s
650s-670s
$ dwfv sample.vcd --when '$! equals h2'
310s-330s
650s-670s
```

- Searching when the `value` signal transitions to `4`:

```shell
$ dwfv sample.vcd --when '$! <- 4'
350s
690s
$ dwfv sample.vcd --when '$! becomes b100'
350s
690s
```

- Searching when the `value` signal transitions to `4` after 400s:

```shell
$ dwfv sample.vcd --when '$! <- 4 and after 400'
690s
```

- Searching when the `value` signal transitions to `4` before 400s:

```shell
$ dwfv sample.vcd --when '$! <- 4 and before 400'
350s
```

LICENSE
-------

[MIT](LICENSE)
