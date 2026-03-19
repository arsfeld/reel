const std = @import("std");
const c = @import("c.zig").c;
const app = @import("app.zig");
const types = @import("../../core/types.zig");

// Rule stored during dialog lifetime
const PendingRule = struct {
    field: [32]u8 = undefined,
    field_len: usize = 0,
    operator: [8]u8 = undefined,
    operator_len: usize = 0,
    value: [128]u8 = undefined,
    value_len: usize = 0,
};

// Dialog state for create collection
const DialogState = struct {
    name_entry: *c.GtkWidget,
    manual_toggle: *c.GtkWidget,
    smart_toggle: *c.GtkWidget,
    rules_section: *c.GtkWidget,
    field_dropdown: *c.GtkWidget,
    operator_dropdown: *c.GtkWidget,
    value_entry: *c.GtkWidget,
    rules_list: *c.GtkWidget,
    dialog_window: *c.GtkWidget,
    rules: [16]PendingRule = undefined,
    rule_count: usize = 0,
};

var dialog_state: ?DialogState = null;

pub const CollectionsView = struct {
    widget: *c.GtkWidget,
    content_stack: *c.GtkWidget,
    list_box: *c.GtkWidget,

    pub fn init() CollectionsView {
        // Outer vertical box: header bar area + content
        const outer_box = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 0);

        // Header area with "New Collection" button
        const header_box = c.gtk_box_new(c.GTK_ORIENTATION_HORIZONTAL, 8);
        c.gtk_widget_set_margin_top(@ptrCast(header_box), 12);
        c.gtk_widget_set_margin_bottom(@ptrCast(header_box), 4);
        c.gtk_widget_set_margin_start(@ptrCast(header_box), 16);
        c.gtk_widget_set_margin_end(@ptrCast(header_box), 16);
        c.gtk_widget_set_halign(@ptrCast(header_box), c.GTK_ALIGN_END);
        c.gtk_widget_set_hexpand(@ptrCast(header_box), 1);

        const new_btn = c.gtk_button_new_with_label("New Collection");
        c.gtk_widget_add_css_class(@ptrCast(new_btn), "suggested-action");
        c.gtk_widget_add_css_class(@ptrCast(new_btn), "pill");
        _ = c.g_signal_connect_data(
            @ptrCast(new_btn),
            "clicked",
            @ptrCast(&onNewCollectionClicked),
            null,
            null,
            c.G_CONNECT_DEFAULT,
        );
        c.gtk_box_append(@ptrCast(header_box), @ptrCast(new_btn));
        c.gtk_box_append(@ptrCast(outer_box), @ptrCast(header_box));

        const content_stack = c.gtk_stack_new();
        c.gtk_widget_set_vexpand(@ptrCast(content_stack), 1);
        c.gtk_widget_set_hexpand(@ptrCast(content_stack), 1);

        // Empty state
        const status = c.adw_status_page_new();
        c.adw_status_page_set_icon_name(@ptrCast(status), "view-grid-symbolic");
        c.adw_status_page_set_title(@ptrCast(status), "No Collections");
        c.adw_status_page_set_description(@ptrCast(status),
            "Create collections to organize your library.",
        );
        _ = c.gtk_stack_add_named(@ptrCast(content_stack), @ptrCast(status), "empty");

        // Collections list
        const scrolled = c.gtk_scrolled_window_new();
        c.gtk_widget_set_vexpand(@ptrCast(scrolled), 1);

        const clamp = c.adw_clamp_new();
        c.adw_clamp_set_maximum_size(@ptrCast(clamp), 800);

        const list_box = c.gtk_list_box_new();
        c.gtk_list_box_set_selection_mode(@ptrCast(list_box), c.GTK_SELECTION_NONE);
        c.gtk_widget_add_css_class(@ptrCast(list_box), "boxed-list");
        c.gtk_widget_set_margin_top(@ptrCast(list_box), 16);
        c.gtk_widget_set_margin_bottom(@ptrCast(list_box), 16);
        c.gtk_widget_set_margin_start(@ptrCast(list_box), 16);
        c.gtk_widget_set_margin_end(@ptrCast(list_box), 16);

        c.adw_clamp_set_child(@ptrCast(clamp), @ptrCast(list_box));
        c.gtk_scrolled_window_set_child(@ptrCast(scrolled), @ptrCast(clamp));
        _ = c.gtk_stack_add_named(@ptrCast(content_stack), scrolled, "list");

        c.gtk_stack_set_visible_child_name(@ptrCast(content_stack), "empty");

        c.gtk_box_append(@ptrCast(outer_box), @ptrCast(content_stack));

        return CollectionsView{
            .widget = @ptrCast(outer_box),
            .content_stack = @ptrCast(content_stack),
            .list_box = @ptrCast(list_box),
        };
    }

    pub fn refresh(self: *CollectionsView) void {
        // Clear existing rows
        while (true) {
            const child = c.gtk_widget_get_first_child(@ptrCast(self.list_box));
            if (child == null) break;
            c.gtk_list_box_remove(@ptrCast(self.list_box), child);
        }

        var lib = app.getLibrary() orelse {
            c.gtk_stack_set_visible_child_name(@ptrCast(self.content_stack), "empty");
            return;
        };

        const collections = lib.listCollections() catch {
            c.gtk_stack_set_visible_child_name(@ptrCast(self.content_stack), "empty");
            return;
        };
        defer lib.freeCollections(collections);

        if (collections.len == 0) {
            c.gtk_stack_set_visible_child_name(@ptrCast(self.content_stack), "empty");
            return;
        }

        for (collections) |col| {
            const allocator = app.getAllocator();
            const name_z = allocator.dupeZ(u8, col.name) catch continue;
            defer allocator.free(name_z);

            const row = c.adw_action_row_new();
            c.adw_preferences_row_set_title(@ptrCast(row), name_z.ptr);

            // Subtitle: type and item count
            const type_str: [*:0]const u8 = if (col.collection_type == .smart) "Smart Collection" else "Collection";
            c.adw_action_row_set_subtitle(@ptrCast(row), type_str);

            const icon_name: [*:0]const u8 = if (col.collection_type == .smart)
                "funnel-symbolic"
            else
                "view-grid-symbolic";
            c.adw_action_row_add_prefix(@ptrCast(row),
                c.gtk_image_new_from_icon_name(icon_name),
            );

            // Chevron suffix
            const chevron = c.gtk_image_new_from_icon_name("go-next-symbolic");
            c.adw_action_row_add_suffix(@ptrCast(row), chevron);
            c.adw_action_row_set_activatable_widget(@ptrCast(row), chevron);

            c.gtk_list_box_append(@ptrCast(self.list_box), @ptrCast(row));
        }

        c.gtk_stack_set_visible_child_name(@ptrCast(self.content_stack), "list");
    }
};

// Global collections view pointer for callbacks
var global_collections_view: ?*CollectionsView = null;

pub fn setGlobalCollectionsView(v: *CollectionsView) void {
    global_collections_view = v;
}

const field_options = [_][*:0]const u8{ "genre", "year", "watched", "media_type", "source" };
const operator_options = [_][*:0]const u8{ "eq", "neq", "gt", "gte", "lt", "lte" };

fn onNewCollectionClicked(_: *c.GtkButton, _: ?*anyopaque) callconv(.c) void {
    const parent_window = app.getWindow();

    // Create dialog window
    const dialog_win = c.gtk_window_new();
    c.gtk_window_set_title(@ptrCast(dialog_win), "New Collection");
    c.gtk_window_set_default_size(@ptrCast(dialog_win), 450, 400);
    c.gtk_window_set_modal(@ptrCast(dialog_win), 1);
    if (parent_window) |pw| {
        c.gtk_window_set_transient_for(@ptrCast(dialog_win), @ptrCast(pw));
    }

    const vbox = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 12);
    c.gtk_widget_set_margin_top(@ptrCast(vbox), 16);
    c.gtk_widget_set_margin_bottom(@ptrCast(vbox), 16);
    c.gtk_widget_set_margin_start(@ptrCast(vbox), 16);
    c.gtk_widget_set_margin_end(@ptrCast(vbox), 16);

    // Name entry
    const name_label = c.gtk_label_new("Name");
    c.gtk_widget_set_halign(@ptrCast(name_label), c.GTK_ALIGN_START);
    c.gtk_box_append(@ptrCast(vbox), @ptrCast(name_label));

    const name_entry = c.gtk_entry_new();
    c.gtk_entry_set_placeholder_text(@ptrCast(name_entry), "Collection name");
    c.gtk_box_append(@ptrCast(vbox), @ptrCast(name_entry));

    // Type toggle: Manual / Smart
    const type_label = c.gtk_label_new("Type");
    c.gtk_widget_set_halign(@ptrCast(type_label), c.GTK_ALIGN_START);
    c.gtk_widget_set_margin_top(@ptrCast(type_label), 8);
    c.gtk_box_append(@ptrCast(vbox), @ptrCast(type_label));

    const toggle_box = c.gtk_box_new(c.GTK_ORIENTATION_HORIZONTAL, 0);
    c.gtk_widget_add_css_class(@ptrCast(toggle_box), "linked");

    const manual_toggle = c.gtk_toggle_button_new_with_label("Manual");
    c.gtk_toggle_button_set_active(@ptrCast(manual_toggle), 1);
    c.gtk_box_append(@ptrCast(toggle_box), @ptrCast(manual_toggle));

    const smart_toggle = c.gtk_toggle_button_new_with_label("Smart");
    c.gtk_toggle_button_set_group(@ptrCast(smart_toggle), @ptrCast(manual_toggle));
    c.gtk_box_append(@ptrCast(toggle_box), @ptrCast(smart_toggle));

    c.gtk_box_append(@ptrCast(vbox), @ptrCast(toggle_box));

    // Rules section (only visible for Smart)
    const rules_section = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 8);
    c.gtk_widget_set_margin_top(@ptrCast(rules_section), 8);
    c.gtk_widget_set_visible(@ptrCast(rules_section), 0);

    const rules_label = c.gtk_label_new("Rules");
    c.gtk_widget_set_halign(@ptrCast(rules_label), c.GTK_ALIGN_START);
    c.gtk_box_append(@ptrCast(rules_section), @ptrCast(rules_label));

    // Rule builder row
    const rule_row = c.gtk_box_new(c.GTK_ORIENTATION_HORIZONTAL, 6);

    // Field dropdown (GtkComboBoxText)
    const field_dropdown = c.gtk_combo_box_text_new();
    for (field_options) |f| {
        c.gtk_combo_box_text_append_text(@ptrCast(field_dropdown), f);
    }
    c.gtk_combo_box_set_active(@ptrCast(field_dropdown), 0);
    c.gtk_box_append(@ptrCast(rule_row), @ptrCast(field_dropdown));

    // Operator dropdown
    const operator_dropdown = c.gtk_combo_box_text_new();
    for (operator_options) |o| {
        c.gtk_combo_box_text_append_text(@ptrCast(operator_dropdown), o);
    }
    c.gtk_combo_box_set_active(@ptrCast(operator_dropdown), 0);
    c.gtk_box_append(@ptrCast(rule_row), @ptrCast(operator_dropdown));

    // Value entry
    const value_entry = c.gtk_entry_new();
    c.gtk_entry_set_placeholder_text(@ptrCast(value_entry), "Value");
    c.gtk_widget_set_hexpand(@ptrCast(value_entry), 1);
    c.gtk_box_append(@ptrCast(rule_row), @ptrCast(value_entry));

    // Add Rule button
    const add_rule_btn = c.gtk_button_new_from_icon_name("list-add-symbolic");
    c.gtk_widget_set_tooltip_text(@ptrCast(add_rule_btn), "Add Rule");
    _ = c.g_signal_connect_data(
        @ptrCast(add_rule_btn),
        "clicked",
        @ptrCast(&onAddRuleClicked),
        null,
        null,
        c.G_CONNECT_DEFAULT,
    );
    c.gtk_box_append(@ptrCast(rule_row), @ptrCast(add_rule_btn));

    c.gtk_box_append(@ptrCast(rules_section), @ptrCast(rule_row));

    // Rules list
    const rules_list = c.gtk_list_box_new();
    c.gtk_list_box_set_selection_mode(@ptrCast(rules_list), c.GTK_SELECTION_NONE);
    c.gtk_widget_add_css_class(@ptrCast(rules_list), "boxed-list");
    c.gtk_box_append(@ptrCast(rules_section), @ptrCast(rules_list));

    c.gtk_box_append(@ptrCast(vbox), @ptrCast(rules_section));

    // Connect smart toggle to show/hide rules section
    _ = c.g_signal_connect_data(
        @ptrCast(smart_toggle),
        "toggled",
        @ptrCast(&onSmartToggled),
        null,
        null,
        c.G_CONNECT_DEFAULT,
    );

    // Spacer
    const spacer = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 0);
    c.gtk_widget_set_vexpand(@ptrCast(spacer), 1);
    c.gtk_box_append(@ptrCast(vbox), @ptrCast(spacer));

    // Create button
    const create_btn = c.gtk_button_new_with_label("Create");
    c.gtk_widget_add_css_class(@ptrCast(create_btn), "suggested-action");
    c.gtk_widget_add_css_class(@ptrCast(create_btn), "pill");
    c.gtk_widget_set_halign(@ptrCast(create_btn), c.GTK_ALIGN_END);
    c.gtk_widget_set_margin_top(@ptrCast(create_btn), 8);
    _ = c.g_signal_connect_data(
        @ptrCast(create_btn),
        "clicked",
        @ptrCast(&onCreateClicked),
        null,
        null,
        c.G_CONNECT_DEFAULT,
    );
    c.gtk_box_append(@ptrCast(vbox), @ptrCast(create_btn));

    c.gtk_window_set_child(@ptrCast(dialog_win), @ptrCast(vbox));

    // Store dialog state
    dialog_state = .{
        .name_entry = @ptrCast(name_entry),
        .manual_toggle = @ptrCast(manual_toggle),
        .smart_toggle = @ptrCast(smart_toggle),
        .rules_section = @ptrCast(rules_section),
        .field_dropdown = @ptrCast(field_dropdown),
        .operator_dropdown = @ptrCast(operator_dropdown),
        .value_entry = @ptrCast(value_entry),
        .rules_list = @ptrCast(rules_list),
        .dialog_window = @ptrCast(dialog_win),
    };

    c.gtk_window_present(@ptrCast(dialog_win));
}

fn onSmartToggled(toggle: *c.GtkToggleButton, _: ?*anyopaque) callconv(.c) void {
    const state = &(dialog_state orelse return);
    const active = c.gtk_toggle_button_get_active(toggle);
    c.gtk_widget_set_visible(state.rules_section, active);
}

fn onAddRuleClicked(_: *c.GtkButton, _: ?*anyopaque) callconv(.c) void {
    const state = &(dialog_state orelse return);
    if (state.rule_count >= 16) return;

    // Read field
    const field_idx = c.gtk_combo_box_get_active(@ptrCast(state.field_dropdown));
    if (field_idx < 0) return;
    const field_str = field_options[@intCast(@as(u32, @bitCast(field_idx)))];

    // Read operator
    const op_idx = c.gtk_combo_box_get_active(@ptrCast(state.operator_dropdown));
    if (op_idx < 0) return;
    const op_str = operator_options[@intCast(@as(u32, @bitCast(op_idx)))];

    // Read value
    const value_buf: [*c]const u8 = c.gtk_editable_get_text(@ptrCast(state.value_entry));
    if (value_buf == null) return;
    const value_str = std.mem.span(value_buf);
    if (value_str.len == 0) return;

    // Store rule
    var rule = &state.rules[state.rule_count];
    const field_span = std.mem.span(field_str);
    const op_span = std.mem.span(op_str);
    @memcpy(rule.field[0..field_span.len], field_span);
    rule.field_len = field_span.len;
    @memcpy(rule.operator[0..op_span.len], op_span);
    rule.operator_len = op_span.len;
    const vlen = @min(value_str.len, rule.value.len);
    @memcpy(rule.value[0..vlen], value_str[0..vlen]);
    rule.value_len = vlen;
    state.rule_count += 1;

    // Add to visible list
    var label_buf: [180]u8 = undefined;
    const label_text = std.fmt.bufPrintZ(&label_buf, "{s} {s} {s}", .{
        field_span,
        op_span,
        value_str,
    }) catch return;
    const row = c.adw_action_row_new();
    c.adw_preferences_row_set_title(@ptrCast(row), label_text.ptr);
    c.gtk_list_box_append(@ptrCast(state.rules_list), @ptrCast(row));

    // Clear value entry
    c.gtk_editable_set_text(@ptrCast(state.value_entry), "");
}

fn onCreateClicked(_: *c.GtkButton, _: ?*anyopaque) callconv(.c) void {
    const state = &(dialog_state orelse return);

    // Read name
    const name_buf: [*c]const u8 = c.gtk_editable_get_text(@ptrCast(state.name_entry));
    if (name_buf == null) return;
    const name_str = std.mem.span(name_buf);
    if (name_str.len == 0) return;

    // Determine type
    const is_smart = c.gtk_toggle_button_get_active(@ptrCast(state.smart_toggle)) != 0;
    const col_type: types.CollectionType = if (is_smart) .smart else .manual;

    // Create collection
    var lib = app.getLibrary() orelse return;
    const col_id = lib.createCollection(name_str, col_type, null) catch |err| {
        std.log.err("Failed to create collection: {}", .{err});
        return;
    };

    // Add rules for smart collections
    if (is_smart) {
        for (state.rules[0..state.rule_count]) |rule| {
            _ = lib.addCollectionRule(
                col_id,
                rule.field[0..rule.field_len],
                rule.operator[0..rule.operator_len],
                rule.value[0..rule.value_len],
            ) catch |err| {
                std.log.err("Failed to add rule: {}", .{err});
            };
        }
    }

    // Close dialog
    c.gtk_window_close(@ptrCast(state.dialog_window));
    dialog_state = null;

    // Refresh collections view
    if (global_collections_view) |view| {
        view.refresh();
    }
}
