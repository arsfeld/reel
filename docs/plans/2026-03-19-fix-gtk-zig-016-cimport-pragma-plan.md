---
title: "fix: GTK @cImport fails with Zig 0.16 due to _Pragma in GLib headers"
type: fix
status: active
date: 2026-03-19
---

# fix: GTK @cImport fails with Zig 0.16 due to _Pragma in GLib headers

## Overview

After switching to Zig 0.16-dev (required for zig-sqlite), the GTK frontend fails to compile with 12,300 errors. The root cause is Zig's translate-c not supporting `_Pragma()` in GLib 2.86 macro definitions. The static library compiles fine — only the GTK exe is affected.

## Problem Statement

Zig's `@cImport` uses translate-c internally. While it correctly handles `_Pragma()` during preprocessing, it also tries to translate macro **definitions** as expressions. GLib 2.86's `gmacros.h` defines deprecation macros using `_Pragma("clang diagnostic push")`, which translate-c cannot parse:

```
glib/gutils.h:327:1: error: unknown type name 'pragma'
gdk/version/gdkversionmacros.h:19:2: error: \"Only <gdk/gdk.h> can be included directly.\"
```

The `__clang__` branch is selected because Zig's translate-c reports itself as clang. This is a known limitation tracked in [ziglang/zig#20405](https://github.com/ziglang/zig/issues/20405).

## Proposed Solution

**Patched header overlays** — copy 2 system headers with `_Pragma` branches disabled, placed before system include paths so they shadow the originals. Plus add `@cDefine` to suppress deprecation warnings.

### Phase 1: Create patched header overlays

Create `include/zig-tc-patches/` with patched versions of two headers:

**`include/zig-tc-patches/glib/gmacros.h`**: Copy from system, change the `_Pragma`-using conditionals to `#if 0`:
```c
// Lines ~748-775 in gmacros.h — force all _Pragma branches to #if 0:
#if 0 /* zig-tc: disabled ICC _Pragma */
...
#elif 0 /* zig-tc: disabled GCC _Pragma */
...
#elif 0 /* zig-tc: disabled MSVC _Pragma */
...
#elif 0 /* zig-tc: disabled clang _Pragma */
...
#else
// This empty fallback is what we want — no-op macros
```

**`include/zig-tc-patches/gdk/version/gdkversionmacros.h`**: Copy from system, remove `#error` guard and disable `_GDK_GNUC_DO_PRAGMA`:
```c
// Remove: #error "Only <gdk/gdk.h> can be included directly."
// Change: #define _GDK_GNUC_DO_PRAGMA(x) _Pragma(G_STRINGIFY (x))
// To:     #define _GDK_GNUC_DO_PRAGMA(x) /* disabled for zig-tc */
```

### Phase 2: Update build.zig

Add the patched include path **before** system library includes:
```zig
exe.root_module.addIncludePath(b.path("include/zig-tc-patches"));
// system libs add their paths after, so patches take priority
exe.root_module.linkSystemLibrary("gtk4", .{});
```

### Phase 3: Add @cDefine to @cImport blocks

Add deprecation warning suppressors to every `@cImport` across all 18 GTK files:
```zig
const c = @cImport({
    @cDefine("GLIB_DISABLE_DEPRECATION_WARNINGS", "1");
    @cDefine("GDK_DISABLE_DEPRECATION_WARNINGS", "1");
    @cInclude("gtk/gtk.h");
    @cInclude("adwaita.h");
});
```

**Bonus improvement:** Consolidate all 18 independent `@cImport` blocks into a single shared `src/apprt/gtk/c.zig`:
```zig
pub usingnamespace @cImport({
    @cDefine("GLIB_DISABLE_DEPRECATION_WARNINGS", "1");
    @cDefine("GDK_DISABLE_DEPRECATION_WARNINGS", "1");
    @cInclude("adwaita.h");
    @cInclude("gtk/gtk.h");
    @cInclude("gdk-pixbuf/gdk-pixbuf.h");
    @cInclude("epoxy/gl.h");
    @cInclude("epoxy/egl.h");
    @cInclude("mpv/client.h");
    @cInclude("mpv/render.h");
    @cInclude("mpv/render_gl.h");
});
```
Then all GTK files replace their `@cImport` block with `const c = @import("c.zig");`.

## Acceptance Criteria

- [ ] `nix develop --command zig build` compiles both static lib AND GTK executable
- [ ] `nix develop --command zig build test` compiles and runs tests
- [ ] Patched headers in `include/zig-tc-patches/` with clear comments explaining why
- [ ] All 18 GTK files updated with deprecation `@cDefine`s (or shared `c.zig`)

## Technical Considerations

- **Header version coupling**: Patched headers must match installed GLib/GTK version. If nixpkgs updates GLib, the patches may need updating. Add a comment in flake.nix documenting this.
- **Minimal patches**: Only disable the `_Pragma` branches — don't modify anything else in the headers.
- **glibc mismatch**: The zig-overlay's master build links glibc 2.40, but some Nix system libs (librist, libssh) need 2.42. Tests compile but fail at runtime with `GLIBC_2.42 not found`. This may need a pinned zig-overlay version or Nix override.

## Sources

- [ziglang/zig#20405 — translate-c pragma operators](https://github.com/ziglang/zig/issues/20405)
- [Ghostty GTK rewrite migrating to zig-gobject](https://mitchellh.com/writing/ghostty-gtk-rewrite)
- [zig-gobject project](https://github.com/ianprime0509/zig-gobject) (long-term alternative)
- GLib source: `glib/gmacros.h` lines 748-775
