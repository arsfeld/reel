/*
 * Wrapper header for Zig translate-c compatibility with GLib 2.86+ / GTK 4.20+.
 *
 * Zig translate-c cannot handle _Pragma() which GLib 2.86 uses extensively.
 * It also appears to process #error directives from raw header source even
 * when they are in non-taken preprocessor branches.
 *
 * Strategy:
 *   1. #define _Pragma(x) to neutralise all _Pragma usage.
 *   2. Disable deprecation warning macros.
 *   3. Pre-include gdk/version/gdkversionmacros.h while __GDK_H_INSIDE__
 *      is still defined, so that the #error guard passes.  Later includes
 *      of the same file are skipped by #pragma once.
 */

#ifndef ZIG_GTK_ALL_H
#define ZIG_GTK_ALL_H

/* Neutralise _Pragma — Zig translate-c chokes on it */
#define _Pragma(x)

/* Disable deprecation-warning macros that use _Pragma */
#define GLIB_DISABLE_DEPRECATION_WARNINGS 1
#define GDK_DISABLE_DEPRECATION_WARNINGS 1

/* Now include all needed headers */
#include <adwaita.h>
#include <gtk/gtk.h>
#include <gdk-pixbuf/gdk-pixbuf.h>
#include <epoxy/gl.h>
#include <epoxy/egl.h>
#include <mpv/client.h>
#include <mpv/render.h>
#include <mpv/render_gl.h>

#endif /* ZIG_GTK_ALL_H */
