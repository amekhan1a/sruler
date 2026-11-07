# sruler

## simple ruler

a wayland screen ruler in rust

it captures the monitor output it's currently in using the compositor's screencopy protocol (no external tools) and measures the region around the cursor

## behavior

- mouse move: move the crosshair and recompute the measurement
- mouse wheel: adjust the color difference threshold T used to detect region edges, lower T = stops at subtler color changes
- shift + mouse wheel: zoom the whole screen in and out around the cursor (1x to 32x)
- middle drag: pan while zoomed
- shift + rmb: point to point measurement, click point A, click point B, click again to start over. ignores the threshold
- ctrl (during point to point): snap the line to horizontal or vertical
- rmb: cancel point to point
- lmb: copy WxH and exit
- esc: cancel point to point, then exit
- h or f1: toggle the help box

sizes are inclusive pixel counts, so clicking the first and last pixel of an element reports the same size the threshold mode would

## configuration
sruler has a config.rs file where you can tweak a small amount of appearance settings, such as scanline color, tooltip font size, tooltip radius, point to point color, whether the help box is open at start and an optional center dot

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
- needs a compositor with `zwlr_layer_shell_v1` and `zwlr_screencopy_manager_v1` (sway, river, labwc, hyprland and other wlroots-based ones). gnome and kde are not supported
- fractional scaling is handled by measuring the real window to frame ratio, but it has seen little testing, so could be janky
- rotated outputs are corrected after capture and are also lightly tested

## license
GPLv3 or later, see LICENSE
