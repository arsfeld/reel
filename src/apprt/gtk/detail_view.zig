const std = @import("std");
const c = @cImport({
    @cInclude("adwaita.h");
    @cInclude("gtk/gtk.h");
});
const app = @import("app.zig");
const types = @import("../../core/types.zig");

pub const DetailView = struct {
    widget: *c.GtkWidget,
    title_label: *c.GtkWidget,
    year_label: *c.GtkWidget,
    summary_label: *c.GtkWidget,
    rating_label: *c.GtkWidget,
    duration_label: *c.GtkWidget,
    type_label: *c.GtkWidget,
    play_button: *c.GtkWidget,
    download_button: *c.GtkWidget,
    fix_match_button: *c.GtkWidget,
    lock_icon: *c.GtkWidget,
    episodes_box: *c.GtkWidget,
    episodes_group: *c.GtkWidget,
    current_item_id: i64 = 0,
    current_source: types.MediaSource = .local,
    current_match_locked: bool = false,
    current_file_path_buf: [1024]u8 = undefined,
    current_file_path_len: usize = 0,

    pub fn init() DetailView {
        const scrolled = c.gtk_scrolled_window_new();
        c.gtk_widget_set_vexpand(@ptrCast(scrolled), 1);
        c.gtk_widget_set_hexpand(@ptrCast(scrolled), 1);

        const clamp = c.adw_clamp_new();
        c.adw_clamp_set_maximum_size(@ptrCast(clamp), 900);

        const vbox = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 16);
        c.gtk_widget_set_margin_top(@ptrCast(vbox), 24);
        c.gtk_widget_set_margin_bottom(@ptrCast(vbox), 24);
        c.gtk_widget_set_margin_start(@ptrCast(vbox), 24);
        c.gtk_widget_set_margin_end(@ptrCast(vbox), 24);

        // Header area: poster placeholder + info
        const header = c.gtk_box_new(c.GTK_ORIENTATION_HORIZONTAL, 24);

        // Poster placeholder
        const poster_frame = c.gtk_frame_new(null);
        c.gtk_widget_set_size_request(@ptrCast(poster_frame), 200, 300);
        c.gtk_widget_set_overflow(@ptrCast(poster_frame), c.GTK_OVERFLOW_HIDDEN);
        const poster_icon = c.gtk_image_new_from_icon_name("camera-video-symbolic");
        c.gtk_image_set_pixel_size(@ptrCast(poster_icon), 64);
        c.gtk_widget_set_opacity(@ptrCast(poster_icon), 0.3);
        c.gtk_widget_set_halign(@ptrCast(poster_icon), c.GTK_ALIGN_CENTER);
        c.gtk_widget_set_valign(@ptrCast(poster_icon), c.GTK_ALIGN_CENTER);
        c.gtk_frame_set_child(@ptrCast(poster_frame), @ptrCast(poster_icon));
        c.gtk_box_append(@ptrCast(header), @ptrCast(poster_frame));

        // Info column
        const info = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 8);
        c.gtk_widget_set_valign(@ptrCast(info), c.GTK_ALIGN_START);
        c.gtk_widget_set_hexpand(@ptrCast(info), 1);

        // Title
        const title_label = c.gtk_label_new("");
        c.gtk_widget_add_css_class(@ptrCast(title_label), "title-1");
        c.gtk_widget_set_halign(@ptrCast(title_label), c.GTK_ALIGN_START);
        c.gtk_label_set_wrap(@ptrCast(title_label), 1);
        c.gtk_label_set_xalign(@ptrCast(title_label), 0);
        c.gtk_box_append(@ptrCast(info), @ptrCast(title_label));

        // Metadata row: year · type · duration · rating
        const meta_box = c.gtk_box_new(c.GTK_ORIENTATION_HORIZONTAL, 12);
        const year_label = c.gtk_label_new("");
        c.gtk_widget_add_css_class(@ptrCast(year_label), "dim-label");
        c.gtk_box_append(@ptrCast(meta_box), @ptrCast(year_label));

        const type_label = c.gtk_label_new("");
        c.gtk_widget_add_css_class(@ptrCast(type_label), "dim-label");
        c.gtk_box_append(@ptrCast(meta_box), @ptrCast(type_label));

        const duration_label = c.gtk_label_new("");
        c.gtk_widget_add_css_class(@ptrCast(duration_label), "dim-label");
        c.gtk_box_append(@ptrCast(meta_box), @ptrCast(duration_label));

        const rating_label = c.gtk_label_new("");
        c.gtk_widget_add_css_class(@ptrCast(rating_label), "dim-label");
        c.gtk_box_append(@ptrCast(meta_box), @ptrCast(rating_label));

        c.gtk_box_append(@ptrCast(info), @ptrCast(meta_box));

        // Play button
        const play_button = c.gtk_button_new_with_label("Play");
        c.gtk_widget_add_css_class(@ptrCast(play_button), "suggested-action");
        c.gtk_widget_add_css_class(@ptrCast(play_button), "pill");
        c.gtk_widget_set_halign(@ptrCast(play_button), c.GTK_ALIGN_START);

        // Connect play button click
        _ = c.g_signal_connect_data(
            @ptrCast(play_button),
            "clicked",
            @ptrCast(&onPlayClicked),
            null,
            null,
            c.G_CONNECT_DEFAULT,
        );

        // Button row (play + download)
        const button_box = c.gtk_box_new(c.GTK_ORIENTATION_HORIZONTAL, 8);
        c.gtk_widget_set_halign(@ptrCast(button_box), c.GTK_ALIGN_START);
        c.gtk_widget_set_margin_top(@ptrCast(button_box), 8);
        c.gtk_box_append(@ptrCast(button_box), @ptrCast(play_button));

        // Download button
        const download_button = c.gtk_button_new_from_icon_name("folder-download-symbolic");
        c.gtk_widget_add_css_class(@ptrCast(download_button), "pill");
        c.gtk_widget_set_tooltip_text(@ptrCast(download_button), "Download for offline viewing");
        c.gtk_widget_set_visible(@ptrCast(download_button), 0); // hidden by default

        _ = c.g_signal_connect_data(
            @ptrCast(download_button),
            "clicked",
            @ptrCast(&onDownloadClicked),
            null,
            null,
            c.G_CONNECT_DEFAULT,
        );

        c.gtk_box_append(@ptrCast(button_box), @ptrCast(download_button));

        // Fix Match button
        const fix_match_button = c.gtk_button_new_from_icon_name("edit-find-replace-symbolic");
        c.gtk_widget_add_css_class(@ptrCast(fix_match_button), "flat");
        c.gtk_widget_set_tooltip_text(@ptrCast(fix_match_button), "Fix Match");
        c.gtk_widget_set_visible(@ptrCast(fix_match_button), 0); // hidden by default

        _ = c.g_signal_connect_data(
            @ptrCast(fix_match_button),
            "clicked",
            @ptrCast(&onFixMatchClicked),
            null,
            null,
            c.G_CONNECT_DEFAULT,
        );

        c.gtk_box_append(@ptrCast(button_box), @ptrCast(fix_match_button));

        // Lock icon (shown when match is locked)
        const lock_icon = c.gtk_image_new_from_icon_name("changes-prevent-symbolic");
        c.gtk_widget_set_tooltip_text(@ptrCast(lock_icon), "Match locked \xe2\x80\x94 rescans will not overwrite metadata");
        c.gtk_widget_set_visible(@ptrCast(lock_icon), 0);
        c.gtk_widget_set_valign(@ptrCast(lock_icon), c.GTK_ALIGN_CENTER);
        c.gtk_box_append(@ptrCast(button_box), @ptrCast(lock_icon));

        c.gtk_box_append(@ptrCast(info), @ptrCast(button_box));

        // Summary
        const summary_label = c.gtk_label_new("");
        c.gtk_widget_set_halign(@ptrCast(summary_label), c.GTK_ALIGN_START);
        c.gtk_label_set_wrap(@ptrCast(summary_label), 1);
        c.gtk_label_set_xalign(@ptrCast(summary_label), 0);
        c.gtk_widget_set_margin_top(@ptrCast(summary_label), 8);
        c.gtk_box_append(@ptrCast(info), @ptrCast(summary_label));

        c.gtk_box_append(@ptrCast(header), @ptrCast(info));
        c.gtk_box_append(@ptrCast(vbox), @ptrCast(header));

        // Episodes section (for TV shows)
        const episodes_group = c.adw_preferences_group_new();
        c.adw_preferences_group_set_title(@ptrCast(episodes_group), "Episodes");
        c.gtk_widget_set_visible(@ptrCast(episodes_group), 0);
        const episodes_box_widget = @as(*c.GtkWidget, @ptrCast(episodes_group));
        c.gtk_box_append(@ptrCast(vbox), episodes_box_widget);

        c.adw_clamp_set_child(@ptrCast(clamp), @ptrCast(vbox));
        c.gtk_scrolled_window_set_child(@ptrCast(scrolled), @ptrCast(clamp));

        return .{
            .widget = @ptrCast(scrolled),
            .title_label = @ptrCast(title_label),
            .year_label = @ptrCast(year_label),
            .summary_label = @ptrCast(summary_label),
            .rating_label = @ptrCast(rating_label),
            .duration_label = @ptrCast(duration_label),
            .type_label = @ptrCast(type_label),
            .play_button = @ptrCast(play_button),
            .download_button = @ptrCast(download_button),
            .fix_match_button = @ptrCast(fix_match_button),
            .lock_icon = @ptrCast(lock_icon),
            .episodes_box = episodes_box_widget,
            .episodes_group = @ptrCast(episodes_group),
        };
    }

    pub fn showItem(self: *DetailView, item: types.MediaItem) void {
        self.current_item_id = item.id;
        self.current_source = item.source;

        // Store file path for play button
        if (item.file_path) |fp| {
            const len = @min(fp.len, self.current_file_path_buf.len - 1);
            @memcpy(self.current_file_path_buf[0..len], fp[0..len]);
            self.current_file_path_buf[len] = 0;
            self.current_file_path_len = len;
        } else {
            self.current_file_path_len = 0;
        }

        // Title
        c.gtk_label_set_text(@ptrCast(self.title_label), @ptrCast(item.title.ptr));

        // Year
        if (item.year) |year| {
            var buf: [16]u8 = undefined;
            const s = std.fmt.bufPrintZ(&buf, "{d}", .{year}) catch "";
            c.gtk_label_set_text(@ptrCast(self.year_label), s);
            c.gtk_widget_set_visible(@ptrCast(self.year_label), 1);
        } else {
            c.gtk_widget_set_visible(@ptrCast(self.year_label), 0);
        }

        // Type
        const type_str: [*:0]const u8 = switch (item.media_type) {
            .movie => "Movie",
            .show => "TV Show",
            .season => "Season",
            .episode => "Episode",
            .other => "Other",
        };
        c.gtk_label_set_text(@ptrCast(self.type_label), type_str);

        // Duration
        if (item.duration_ms) |ms| {
            const minutes = @divTrunc(ms, 60000);
            const hours = @divTrunc(minutes, 60);
            const mins = @rem(minutes, 60);
            var buf: [32]u8 = undefined;
            const s = if (hours > 0)
                std.fmt.bufPrintZ(&buf, "{d}h {d}m", .{ hours, mins }) catch ""
            else
                std.fmt.bufPrintZ(&buf, "{d}m", .{mins}) catch "";
            c.gtk_label_set_text(@ptrCast(self.duration_label), s);
            c.gtk_widget_set_visible(@ptrCast(self.duration_label), 1);
        } else {
            c.gtk_widget_set_visible(@ptrCast(self.duration_label), 0);
        }

        // Rating
        if (item.rating) |r| {
            var buf: [16]u8 = undefined;
            const s = std.fmt.bufPrintZ(&buf, "{d:.1}/10", .{r}) catch "";
            c.gtk_label_set_text(@ptrCast(self.rating_label), s);
            c.gtk_widget_set_visible(@ptrCast(self.rating_label), 1);
        } else {
            c.gtk_widget_set_visible(@ptrCast(self.rating_label), 0);
        }

        // Summary
        if (item.summary) |summary| {
            c.gtk_label_set_text(@ptrCast(self.summary_label), @ptrCast(summary.ptr));
            c.gtk_widget_set_visible(@ptrCast(self.summary_label), 1);
        } else {
            c.gtk_widget_set_visible(@ptrCast(self.summary_label), 0);
        }

        // Play button visibility
        c.gtk_widget_set_visible(@ptrCast(self.play_button),
            if (item.file_path != null) 1 else 0);

        // Download button: show for Plex items (movies/episodes) that have a file path
        const show_download = item.source == .plex and item.file_path != null and
            (item.media_type == .movie or item.media_type == .episode);
        c.gtk_widget_set_visible(@ptrCast(self.download_button), if (show_download) 1 else 0);

        // Update download button state based on existing downloads
        if (show_download) {
            self.updateDownloadButton();
        }

        // Fix Match button: show for movies and shows
        self.current_match_locked = item.match_locked;
        const show_fix_match = item.media_type == .movie or item.media_type == .show;
        c.gtk_widget_set_visible(@ptrCast(self.fix_match_button), if (show_fix_match) 1 else 0);
        c.gtk_widget_set_visible(@ptrCast(self.lock_icon), if (item.match_locked) 1 else 0);

        // Episodes for TV shows
        if (item.media_type == .show) {
            self.loadEpisodes(item.id);
            c.gtk_widget_set_visible(self.episodes_box, 1);
        } else {
            c.gtk_widget_set_visible(self.episodes_box, 0);
        }
    }

    fn updateDownloadButton(self: *DetailView) void {
        var dl = app.getDownloader() orelse return;
        const existing = dl.getByMediaItemId(self.current_item_id) catch return orelse {
            // No existing download — show download icon
            c.gtk_button_set_icon_name(@ptrCast(self.download_button), "folder-download-symbolic");
            c.gtk_widget_set_tooltip_text(@ptrCast(self.download_button), "Download for offline viewing");
            c.gtk_widget_set_sensitive(@ptrCast(self.download_button), 1);
            return;
        };
        defer dl.freeDownload(existing);

        switch (existing.status) {
            .complete => {
                c.gtk_button_set_icon_name(@ptrCast(self.download_button), "emblem-ok-symbolic");
                c.gtk_widget_set_tooltip_text(@ptrCast(self.download_button), "Downloaded");
                c.gtk_widget_set_sensitive(@ptrCast(self.download_button), 0);
            },
            .downloading, .queued => {
                c.gtk_button_set_icon_name(@ptrCast(self.download_button), "content-loading-symbolic");
                c.gtk_widget_set_tooltip_text(@ptrCast(self.download_button), "Downloading...");
                c.gtk_widget_set_sensitive(@ptrCast(self.download_button), 0);
            },
            .paused => {
                c.gtk_button_set_icon_name(@ptrCast(self.download_button), "media-playback-pause-symbolic");
                c.gtk_widget_set_tooltip_text(@ptrCast(self.download_button), "Download paused");
                c.gtk_widget_set_sensitive(@ptrCast(self.download_button), 0);
            },
            .failed => {
                c.gtk_button_set_icon_name(@ptrCast(self.download_button), "folder-download-symbolic");
                c.gtk_widget_set_tooltip_text(@ptrCast(self.download_button), "Retry download");
                c.gtk_widget_set_sensitive(@ptrCast(self.download_button), 1);
            },
        }
    }

    fn loadEpisodes(self: *DetailView, show_id: i64) void {
        var lib = app.getLibrary() orelse return;

        // Get seasons
        const seasons = lib.getItemsByParent(show_id) catch return;
        defer lib.freeMediaItems(seasons);

        // Clear existing episodes
        // For simplicity, we add season rows directly to the group
        for (seasons) |season| {
            const row = c.adw_action_row_new();
            c.adw_preferences_row_set_title(@ptrCast(row), @ptrCast(season.title.ptr));
            c.adw_action_row_add_prefix(@ptrCast(row),
                c.gtk_image_new_from_icon_name("view-list-symbolic"),
            );
            c.adw_action_row_add_suffix(@ptrCast(row),
                c.gtk_image_new_from_icon_name("go-next-symbolic"),
            );
            c.adw_preferences_group_add(@ptrCast(self.episodes_group), @ptrCast(row));
        }
    }
};

// Global detail view pointer for the play button callback
var global_detail: ?*DetailView = null;

pub fn setGlobalDetail(d: *DetailView) void {
    global_detail = d;
}

fn onPlayClicked(_: *c.GtkButton, _: ?*anyopaque) callconv(.c) void {
    const detail = global_detail orelse return;
    if (detail.current_file_path_len == 0) return;

    const path = detail.current_file_path_buf[0..detail.current_file_path_len];
    app.playMediaItem(detail.current_item_id, path);
}

fn onDownloadClicked(_: *c.GtkButton, _: ?*anyopaque) callconv(.c) void {
    const detail = global_detail orelse return;
    if (detail.current_item_id == 0) return;

    app.enqueueDownload(detail.current_item_id);

    // Update button state immediately
    detail.updateDownloadButton();
}

fn onFixMatchClicked(_: *c.GtkButton, _: ?*anyopaque) callconv(.c) void {
    const detail = global_detail orelse return;
    if (detail.current_item_id == 0) return;

    // If match is locked, toggle unlock
    if (detail.current_match_locked) {
        var lib = app.getLibrary() orelse return;
        lib.setMatchLocked(detail.current_item_id, false) catch return;
        detail.current_match_locked = false;
        c.gtk_widget_set_visible(@ptrCast(detail.lock_icon), 0);
        return;
    }

    // TODO: Open TMDB search dialog for match correction
    // For now, log that the button was clicked
    std.log.info("Fix Match clicked for item {d}", .{detail.current_item_id});
}
