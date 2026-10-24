# Why Calculator started slowly, and what fixed it

Measured on the author's machine (Arch Linux, Hyprland, Papirus-Dark icon
theme), from launch to the first frame on screen, with
`CALCULATOR_TIMING=1 CALCULATOR_QUIT_ON_FIRST_FRAME=1`.

## Short answer

It was not Rust. Nearly all the time went to GTK and to things GTK waits on
when it starts: the icon theme and the graphics renderer. The calculator's
own code (settings, colours, palette, building the window) takes about
15 ms.

| | First frame |
| --- | --- |
| 2.0.2 | about 400 ms |
| After the changes below | about 100 ms |

## Where the time went (2.0.2)

| Step | Time | Whose |
| --- | --- | --- |
| GTK itself starts (display, settings) | 6 ms | GTK |
| App registers on D-Bus | under 1 ms | GLib |
| GtkApplication start-up: **waits for the icon theme** | 350 ms | GTK + icon theme |
| Icon font, settings, history, colours, window | 15 ms | Calculator |
| First layout and drawing, **Vulkan renderer** setup | 45 ms + ~115 ms | GTK + GPU driver |

## The causes

1. **The icon theme (about 325 ms).** Any lookup in the icon theme waits
   until the whole theme has loaded. Papirus is very large; without an
   up-to-date `icon-theme.cache` GTK scans thousands of files. GTK looks up
   icons three times before a window first appears:
   - GtkApplication checks for an icon named after the app;
   - the window looks up its own icon when it is first shown;
   - the header bar's minimize, maximize and close buttons.

   Every GTK app pays this on such a system, not just this one.

2. **The Vulkan renderer (about 115 ms).** Newer GTK versions draw with
   Vulkan by default. Setting it up took longer than GTK's GL renderer on
   this machine.

3. **A long history (30–70 ms).** All history rows were built before the
   window appeared, though the history sheet is closed at start.

Accessibility (`GTK_A11Y=none`), input methods (`GTK_IM_MODULE=simple`),
portals (`GDK_DEBUG=no-portals`) and D-Bus made no difference.

## What changed

- The app sets its icon name before GTK starts, which skips GTK's check;
  clears it while the window first shows; and sets it again after the first
  frame, when the theme has loaded in the background.
- The window buttons are drawn with the app's own icon font, like its other
  icons, so the window never needs the icon theme to appear.
- GTK's GL renderer is used unless `GSK_RENDERER` says otherwise.
- History rows are built just after the window appears, or when the history
  sheet opens, whichever comes first.

## Is this Rust-specific?

No. The Python version of Calculator (1.x) had the same GTK waits on top of
starting Python itself: on a test machine it reached its first frame in
about 400 ms against about 220 ms for the Rust version, before any of these
fixes. Rust removed the interpreter's start-up cost; the rest was GTK and
the system around it, and any GTK 4 app on the same machine pays it unless
it works around it like this.

## Making every GTK app start faster

If `CALCULATOR_TIMING` shows a long wait in other GTK apps too, check that
your icon theme has a cache, and rebuild it if not (fish or bash):

```sh
ls /usr/share/icons/Papirus-Dark/icon-theme.cache
sudo gtk-update-icon-cache -f /usr/share/icons/Papirus-Dark
```

To try the renderers on your machine:

```sh
GSK_RENDERER=vulkan CALCULATOR_TIMING=1 CALCULATOR_QUIT_ON_FIRST_FRAME=1 io.github.HypestHyper.Calculator
GSK_RENDERER=ngl    CALCULATOR_TIMING=1 CALCULATOR_QUIT_ON_FIRST_FRAME=1 io.github.HypestHyper.Calculator
```
