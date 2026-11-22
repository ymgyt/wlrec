# wlrec

A CLI window recorder for Wayland.

```sh
nix develop
just run demo.gif
```

Pick a window. Press Ctrl-C to save the GIF.

For scripts, wait for `ready` on stdout, then send `SIGINT` to stop recording.
