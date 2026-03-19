pub const c = @cImport({
    @cDefine("GLIB_DISABLE_DEPRECATION_WARNINGS", "1");
    @cDefine("GDK_DISABLE_DEPRECATION_WARNINGS", "1");
    @cDefine("_Pragma(x)", {});
    @cInclude("adwaita.h");
    @cInclude("gtk/gtk.h");
    @cInclude("gdk-pixbuf/gdk-pixbuf.h");
    @cInclude("epoxy/gl.h");
    @cInclude("epoxy/egl.h");
    @cInclude("mpv/client.h");
    @cInclude("mpv/render.h");
    @cInclude("mpv/render_gl.h");
});
