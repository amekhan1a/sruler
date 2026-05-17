# sruler

## simple ruler

a wayland screen ruler in rust

it takes a screenshot of the monitor output it's currently in via `grim` and measures the region around the cursor

## behavior

- mouse move: move the crosshair and recompute the measurement
- mouse wheel: adjust the color difference threshold T used to detect region edges, lower T = stops at subtler color changes
- lmb: copy WxH and exit
- esc: exit

## configuration
sruler has a config.rs file where you can tweak a small amount of appearance settings, such as scanline color, tooltip radius and an optional center dot

every change you make in src/config.rs requires a rebuild

## build

install `grim`, then:

```bash
cargo build --release
```

## run

```bash
./target/release/sruler
```

## notes

- wayland only
- fractional scaling makes it janky

## license
GPLv3 or later, see LICENSE
