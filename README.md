# sruler

## simple ruler

a wayland screen ruler in rust

it captures a single screenshot through the XDG desktop screenshot portal, opens a fullscreen transparent overlay, and measures the region around the cursor

## behavior

- mouse move: move the crosshair and recompute the measurement
- mouse wheel: adjust the color difference threshold T used to detect region edges, lower T = stops at subtler color changes
- lmb: copy WxH and exit
- esc: exit

## configuration
sruler has a config.rs file where you can tweak a small amount of appearance settings, such as scanline color, tooltip radius and an optional center dot

every change you make in src/config.rs requires a rebuild

## build

```bash
cargo build --release
```

## run

```bash
./target/release/sruler
```

## notes

- wayland only
- screen capture requires a desktop portal implementation that supports the screenshot portal
- because it uses desktop portals, it may save a screenshot every use of the program, sorry
- no multi monitor support, it literally doesn't work if more than one monitor is on

## license
GPLv3 or later, see LICENSE
