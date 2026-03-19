/* gdkversionmacros.h - version boundaries checks
 * Copyright (C) 2012 Red Hat, Inc.
 *
 * This library is free software; you can redistribute it and/or
 * modify it under the terms of the GNU Lesser General Public
 * License as published by the Free Software Foundation; either
 * version 2 of the License, or (at your option) any later version.
 *
 * This library is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.▸ See the GNU
 * Lesser General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public
 * License along with this library; if not, see <http://www.gnu.org/licenses/>.
 */

#if 0 /* disabled for zig translate-c */
#error "Only <gdk/gdk.h> can be included directly."
#endif

#pragma once

#include <glib.h>

/* These macros are used to mark deprecated symbols in GLib headers,
 * and thus have to be exposed in installed headers. But please
 * do *not* use them in other projects. Instead define your own wrappers
 * around it.
 */

#if 0 /* disabled for zig translate-c — _Pragma not supported */
#define _GDK_GNUC_DO_PRAGMA(x) _Pragma(G_STRINGIFY (x))
#define GDK_DEPRECATED_MACRO _GDK_GNUC_DO_PRAGMA(GCC warning "Deprecated pre-processor symbol")
#define GDK_DEPRECATED_MACRO_FOR(f) \
  _GDK_GNUC_DO_PRAGMA(GCC warning G_STRINGIFY (Deprecated pre-processor symbol: replace with #f))
#define GDK_UNAVAILABLE_MACRO(maj,min) \
  _GDK_GNUC_DO_PRAGMA(GCC warning G_STRINGIFY (Not available before maj.min))
#else
#define _GDK_GNUC_DO_PRAGMA(x) /* disabled for zig translate-c */
#define GDK_DEPRECATED_MACRO
#define GDK_DEPRECATED_MACRO_FOR(f)
#define GDK_UNAVAILABLE_MACRO(maj,min)
#endif

#if !defined(GDK_DISABLE_DEPRECATION_WARNINGS) && \
    (G_GNUC_CHECK_VERSION(6, 1) ||                 \
     (defined (__clang_major__) && (__clang_major__ > 3 || (__clang_major__ == 3 && __clang_minor__ >= 0))))
#define GDK_DEPRECATED_ENUMERATOR G_DEPRECATED
#define GDK_DEPRECATED_ENUMERATOR_FOR(f) G_DEPRECATED_FOR(f)
#define GDK_UNAVAILABLE_ENUMERATOR(maj,min) G_UNAVAILABLE(maj,min)
#else
#define GDK_DEPRECATED_ENUMERATOR
#define GDK_DEPRECATED_ENUMERATOR_FOR(f)
#define GDK_UNAVAILABLE_ENUMERATOR(maj,min)
#endif

#if !defined(GDK_DISABLE_DEPRECATION_WARNINGS) && \
    (G_GNUC_CHECK_VERSION(3, 1) ||                 \
     (defined (__clang_major__) && (__clang_major__ > 3 || (__clang_major__ == 3 && __clang_minor__ >= 0))))
#define GDK_DEPRECATED_TYPE G_DEPRECATED
#define GDK_DEPRECATED_TYPE_FOR(f) G_DEPRECATED_FOR(f)
#define GDK_UNAVAILABLE_TYPE(maj,min) G_UNAVAILABLE(maj,min)
#else
#define GDK_DEPRECATED_TYPE
#define GDK_DEPRECATED_TYPE_FOR(f)
#define GDK_UNAVAILABLE_TYPE(maj,min)
#endif

/**
 * GDK_MAJOR_VERSION:
 *
 * The major version component of the library's version, e.g. "1" for "1.2.3".
 */
#define GDK_MAJOR_VERSION (4)

/**
 * GDK_MINOR_VERSION:
 *
 * The minor version component of the library's version, e.g. "2" for "1.2.3".
 */
#define GDK_MINOR_VERSION (20)

/**
 * GDK_MICRO_VERSION:
 *
 * The micro version component of the library's version, e.g. "3" for "1.2.3".
 */
#define GDK_MICRO_VERSION (3)
/**
 * GDK_VERSION_4_0:
 *
 * A macro that evaluates to the 4.0 version of GTK, in a format
 * that can be used by the C pre-processor.
 *
 * Since: 4.0
 */
#define GDK_VERSION_4_0 (G_ENCODE_VERSION (4, 0))
/**
 * GDK_VERSION_4_2:
 *
 * A macro that evaluates to the 4.2 version of GTK, in a format
 * that can be used by the C pre-processor.
 *
 * Since: 4.2
 */
#define GDK_VERSION_4_2 (G_ENCODE_VERSION (4, 2))
/**
 * GDK_VERSION_4_4:
 *
 * A macro that evaluates to the 4.4 version of GTK, in a format
 * that can be used by the C pre-processor.
 *
 * Since: 4.4
 */
#define GDK_VERSION_4_4 (G_ENCODE_VERSION (4, 4))
/**
 * GDK_VERSION_4_6:
 *
 * A macro that evaluates to the 4.6 version of GTK, in a format
 * that can be used by the C pre-processor.
 *
 * Since: 4.6
 */
#define GDK_VERSION_4_6 (G_ENCODE_VERSION (4, 6))
/**
 * GDK_VERSION_4_8:
 *
 * A macro that evaluates to the 4.8 version of GTK, in a format
 * that can be used by the C pre-processor.
 *
 * Since: 4.8
 */
#define GDK_VERSION_4_8 (G_ENCODE_VERSION (4, 8))
/**
 * GDK_VERSION_4_10:
 *
 * A macro that evaluates to the 4.10 version of GTK, in a format
 * that can be used by the C pre-processor.
 *
 * Since: 4.10
 */
#define GDK_VERSION_4_10 (G_ENCODE_VERSION (4, 10))
/**
 * GDK_VERSION_4_12:
 *
 * A macro that evaluates to the 4.12 version of GTK, in a format
 * that can be used by the C pre-processor.
 *
 * Since: 4.12
 */
#define GDK_VERSION_4_12 (G_ENCODE_VERSION (4, 12))
/**
 * GDK_VERSION_4_14:
 *
 * A macro that evaluates to the 4.14 version of GTK, in a format
 * that can be used by the C pre-processor.
 *
 * Since: 4.14
 */
#define GDK_VERSION_4_14 (G_ENCODE_VERSION (4, 14))
/**
 * GDK_VERSION_4_16:
 *
 * A macro that evaluates to the 4.16 version of GTK, in a format
 * that can be used by the C pre-processor.
 *
 * Since: 4.16
 */
#define GDK_VERSION_4_16 (G_ENCODE_VERSION (4, 16))
/**
 * GDK_VERSION_4_18:
 *
 * A macro that evaluates to the 4.18 version of GTK, in a format
 * that can be used by the C pre-processor.
 *
 * Since: 4.18
 */
#define GDK_VERSION_4_18 (G_ENCODE_VERSION (4, 18))
/**
 * GDK_VERSION_4_20:
 *
 * A macro that evaluates to the 4.20 version of GTK, in a format
 * that can be used by the C pre-processor.
 *
 * Since: 4.20
 */
#define GDK_VERSION_4_20 (G_ENCODE_VERSION (4, 20))

/* evaluates to the current stable version; for development cycles,
 * this means the next stable target, with a hard backstop to the
 * beginning of the stable series
 */
#if GDK_MAJOR_VERSION >= 4 && (GDK_MINOR_VERSION % 2)
# define GDK_VERSION_CUR_STABLE         (G_ENCODE_VERSION (GDK_MAJOR_VERSION, GDK_MINOR_VERSION + 1))
#elif G_ENCODE_VERSION (GDK_MAJOR_VERSION, GDK_MINOR_VERSION) > GDK_VERSION_4_0
# define GDK_VERSION_CUR_STABLE         (G_ENCODE_VERSION (GDK_MAJOR_VERSION, GDK_MINOR_VERSION))
#else
# define GDK_VERSION_CUR_STABLE         GDK_VERSION_4_0
#endif

/* evaluates to the previous stable version, with a hard backstop
 * to the beginning of the stable series
 */
#if GDK_MAJOR_VERSION >= 4 && (GDK_MINOR_VERSION % 2)
# define GDK_VERSION_PREV_STABLE        (G_ENCODE_VERSION (GDK_MAJOR_VERSION, GDK_MINOR_VERSION - 1))
#elif GDK_MAJOR_VERSION >= 4 && GDK_MINOR_VERSION > 2
# define GDK_VERSION_PREV_STABLE        (G_ENCODE_VERSION (GDK_MAJOR_VERSION, GDK_MINOR_VERSION - 2))
#else
# define GDK_VERSION_PREV_STABLE        GDK_VERSION_4_0
#endif

/**
 * GDK_VERSION_MIN_REQUIRED:
 *
 * A macro that should be defined by the user prior to including
 * the `gdk.h` header.
 *
 * The definition should be one of the predefined GDK version
 * macros: %GDK_VERSION_4_0, %GDK_VERSION_4_2,...
 *
 * This macro defines the lower bound for the GDK API to use.
 *
 * If a function has been deprecated in a newer version of GDK,
 * it is possible to use this symbol to avoid the compiler warnings
 * without disabling warning for every deprecated function.
 */
#ifndef GDK_VERSION_MIN_REQUIRED
# define GDK_VERSION_MIN_REQUIRED      (GDK_VERSION_CUR_STABLE)
#endif

/**
 * GDK_VERSION_MAX_ALLOWED:
 *
 * A macro that should be defined by the user prior to including
 * the `gdk.h` header.
 *
 * The definition should be one of the predefined GDK version
 * macros: %GDK_VERSION_4_0, %GDK_VERSION_4_2,...
 *
 * This macro defines the upper bound for the GDK API to use.
 *
 * If a function has been introduced in a newer version of GDK,
 * it is possible to use this symbol to get compiler warnings when
 * trying to use that function.
 */
#ifndef GDK_VERSION_MAX_ALLOWED
# if GDK_VERSION_MIN_REQUIRED > GDK_VERSION_PREV_STABLE
#  define GDK_VERSION_MAX_ALLOWED      GDK_VERSION_MIN_REQUIRED
# else
#  define GDK_VERSION_MAX_ALLOWED      GDK_VERSION_CUR_STABLE
# endif
#endif

/* sanity checks */
#if GDK_VERSION_MAX_ALLOWED < GDK_VERSION_MIN_REQUIRED
# error "GDK_VERSION_MAX_ALLOWED must be >= GDK_VERSION_MIN_REQUIRED"
#endif
#if GDK_VERSION_MIN_REQUIRED < GDK_VERSION_4_0
# error "GDK_VERSION_MIN_REQUIRED must be >= GDK_VERSION_4_0"
#endif

#include <gdk/version/gdk-visibility.h>
