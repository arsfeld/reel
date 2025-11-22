---
id: task-335
title: 'Fix search page navigation crash: widget already has parent'
status: Done
assignee:
  - '@claude'
created_date: '2025-10-02 19:17'
updated_date: '2025-10-02 19:31'
labels:
  - bug
  - search
  - ui
  - navigation
dependencies: []
priority: high
---

## Description

Search page navigation sometimes fails with 'adw_navigation_page_set_child: assertion gtk_widget_get_parent (child) == NULL failed'. This occurs because the SearchPage widget is being reused in multiple NavigationPage instances, violating GTK's single-parent constraint.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify root cause: SearchPage controller widget being added to multiple parents
- [x] #2 Fix: Either reuse same NavigationPage or create new SearchPage controller on each navigation
- [x] #3 Ensure search page can be navigated to multiple times without errors
- [x] #4 Test: Navigate away from search, then back to search repeatedly
- [x] #5 Verify no GTK critical warnings in console
<!-- AC:END -->


## Implementation Plan

1. Analyze current navigation flow and widget parent relationships
2. Implement fix: Unparent widget from old NavigationPage before creating new one
3. Test navigation to search multiple times
4. Verify no GTK warnings in console
5. Update implementation notes with solution


## Implementation Notes

Fixed search page navigation crash by unparenting the SearchPage widget from old NavigationPage before creating new one.


## Solution
Added code in src/ui/main_window.rs:1209-1211 to call `old_page.set_child(None::<&gtk::Widget>)` on the previous NavigationPage before creating a new one with the SearchPage widget.

## Root Cause
The SearchPage controller widget was being reused across multiple NavigationPage instances. GTK enforces single-parent constraint, causing the assertion failure when trying to add a widget that already has a parent.

## Changes
- Modified NavigateToSearch handler to unparent widget before reuse
- No changes needed to SearchPage component itself
- Pattern can be applied to other similar navigation scenarios


## Root Cause Analysis

**Current Code (main_window.rs ~1244-1253):**
```rust
// Always create a new NavigationPage (can't reuse after pop)
if let Some(ref search_controller) = self.search_page {
    let page = adw::NavigationPage::builder()
        .title("Search")
        .child(search_controller.widget())  // <-- PROBLEM HERE
        .build();

    self.navigation_view.push(&page);
    self.search_nav_page = Some(page);
}
```

**The Issue:**
1. First navigation: Creates SearchPage controller, widget has no parent ✅
2. Create NavigationPage with controller.widget() as child ✅
3. Push to navigation_view - widget now has parent ✅
4. User navigates away - NavigationPage is popped
5. **Second navigation:** Try to create NEW NavigationPage with SAME widget
6. **GTK Error:** Widget still has parent from previous NavigationPage ❌

**Why this happens:**
- `search_controller.widget()` returns the SAME widget instance every time
- GTK widgets can only have ONE parent at a time
- We're trying to add the same widget to multiple NavigationPage instances

## Potential Solutions

### Option A: Never create new NavigationPage (Recommended)
Only create NavigationPage once, reuse it:
```rust
if self.search_nav_page.is_none() {
    let page = adw::NavigationPage::builder()
        .title("Search")
        .child(search_controller.widget())
        .build();
    self.search_nav_page = Some(page);
}

// Just push the existing page
if let Some(ref page) = self.search_nav_page {
    self.navigation_view.push(page);
}
```

**Pros:** Simple, efficient, matches how other pages work
**Cons:** May have Adwaita-specific issues with re-pushing

### Option B: Unparent widget before reuse
```rust
if let Some(ref search_controller) = self.search_page {
    let widget = search_controller.widget();
    
    // Remove from previous parent if exists
    if let Some(parent) = widget.parent() {
        parent.downcast::<adw::NavigationPage>()
            .unwrap()
            .set_child(gtk::Widget::NONE);
    }
    
    let page = adw::NavigationPage::builder()
        .title("Search")
        .child(widget)
        .build();
    // ...
}
```

**Pros:** Explicit control
**Cons:** More complex, error-prone

### Option C: Create new SearchPage controller each time
```rust
// Don't cache search_page, create fresh each time
let search_controller = SearchPage::builder()
    .launch(self.db.clone())
    .forward(/* ... */);

let page = adw::NavigationPage::builder()
    .title("Search")
    .child(search_controller.widget())
    .build();
```

**Pros:** Clean slate each time
**Cons:** Loses search state, wasteful

## Recommendation
**Use Option A** - Create NavigationPage once and reuse it. This matches the pattern used by other pages in the app and is the simplest solution.

## Testing Steps
1. Launch app
2. Search for something → Navigate to search page
3. Click back button
4. Search again → Navigate to search page  
5. Repeat 5-10 times
6. **Expected:** No GTK warnings, page works every time
7. **Currently:** GTK warning appears on 2nd+ navigation

## Files to Modify
- `src/ui/main_window.rs` (lines ~1229-1253) - NavigateToSearch handler
