//! Single-pass layout with explicit containers.
//!
//! Each container pushes a [`Layout`] describing its content region and a
//! cursor. Items are placed in order; when a container ends, its measured
//! content size is placed in the parent. Containers whose background depends
//! on the content size (cards) reserve draw-list slots up front and fill them
//! at `end`, so backgrounds are still drawn beneath their content without a
//! second layout pass.

use crate::{Response, Sense, Ui, UiError};
use rugi_core::draw::{DrawList, Fill, Slot};
use rugi_core::{Id, Rect, Vec2};

/// Horizontal alignment for rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Align {
    /// Items flow left to right.
    #[default]
    Start,
    /// Items flow right to left, starting at the right edge.
    End,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Dir {
    Vertical,
    Horizontal(Align),
    /// A row whose items are centered on the cross axis.
    CenteredRow(Align),
    Grid(u32),
}

impl Dir {
    /// A left-to-right row, centered or not.
    #[inline]
    fn is_row_start(self) -> bool {
        matches!(self, Dir::Horizontal(Align::Start) | Dir::CenteredRow(Align::Start))
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Container {
    Root,
    Plain,
    /// A group with an explicit width, which it keeps inside rows too.
    FixedWidth,
    Panel,
    Card {
        shadow: Slot,
        bg: Slot,
        padding: f32,
        min_height: f32,
        elevated: bool,
        glass: Option<rugi_core::GlassStyle>,
    },
    /// Screen transition: content faded (and offset) while a page appears.
    Transition {
        start: usize,
        fade: f32,
        offset: Vec2,
    },
    Scroll {
        id: Id,
        outer: Rect,
        offset: f32,
    },
    /// Floating popup / modal (does not take space in its parent).
    Popup {
        id: Id,
        shadow: Slot,
        bg: Slot,
        origin: Vec2,
        width: f32,
        pad: f32,
        kind: crate::popup::PopupKind,
        start: usize,
    },
    /// Explicit rectangle (table cells); does not allocate in its parent.
    Region,
    /// Row whose items are centered on the cross axis within `height`, the
    /// row's height measured in the previous frame. `forced_y` places the next
    /// item at an explicit top edge: a child container is allocated where its
    /// content was laid out.
    CenteredRow {
        id: Id,
        height: f32,
        forced_y: Option<f32>,
    },
    /// Collapsible section body, clipped to an animated height.
    Collapse {
        id: Id,
        open_t: f32,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct Layout {
    pub dir: Dir,
    pub region: Rect,
    pub cursor: Vec2,
    pub spacing: f32,
    pub row_h: f32,
    pub col: u32,
    pub count: u32,
    pub extent: Vec2,
    pub container: Container,
}

#[derive(Default)]
pub(crate) struct CollapseHeight(pub f32);

/// Child containers of a centered row whose height is remembered; later
/// ones are top-aligned (leaf widgets are always centered).
const ROW_REMEMBERED_ITEMS: usize = 8;

/// Heights a centered row measured in the previous frame.
#[derive(Default)]
struct RowHeights {
    /// Height of the whole row.
    row: Option<f32>,
    /// Outer height of each child container, by item index (0 = unknown).
    items: [f32; ROW_REMEMBERED_ITEMS],
}

/// Largest size, spacing or coordinate accepted from callers (logical px).
pub(crate) const MAX_EXTENT: f32 = 1.0e6;

/// A caller-supplied size clamped to `0..=MAX_EXTENT`, with NaN as zero.
#[inline]
#[expect(clippy::manual_clamp, reason = "`clamp` keeps NaN; `max` maps it to 0 without a branch")]
pub(crate) fn extent(v: f32) -> f32 {
    v.max(0.0).min(MAX_EXTENT)
}

impl Layout {
    pub fn new(dir: Dir, region: Rect, spacing: f32, container: Container) -> Self {
        let cursor = match dir {
            Dir::Horizontal(Align::End) | Dir::CenteredRow(Align::End) => Vec2::new(region.max.x, region.min.y),
            _ => region.min,
        };
        Layout { dir, region, cursor, spacing, row_h: 0.0, col: 0, count: 0, extent: Vec2::ZERO, container }
    }

    pub fn root(screen: Rect, spacing: f32) -> Self {
        Self::new(Dir::Vertical, screen, spacing, Container::Root)
    }

    fn col_width(&self, n: u32) -> f32 {
        ((self.region.width() - self.spacing * (n.max(1) - 1) as f32) / n.max(1) as f32).max(0.0)
    }

    /// Where the next item would start and how wide it may be.
    #[inline(always)]
    pub fn peek(&self) -> (Vec2, f32) {
        let gap = if self.count > 0 { self.spacing } else { 0.0 };
        match self.dir {
            Dir::Vertical => (Vec2::new(self.region.min.x, self.cursor.y + gap), self.region.width()),
            Dir::Horizontal(Align::Start) | Dir::CenteredRow(Align::Start) => {
                let x = self.cursor.x + gap;
                (Vec2::new(x, self.region.min.y), (self.region.max.x - x).max(0.0))
            }
            Dir::Horizontal(Align::End) | Dir::CenteredRow(Align::End) => {
                let x = self.cursor.x - gap;
                (Vec2::new(self.region.min.x, self.region.min.y), (x - self.region.min.x).max(0.0))
            }
            Dir::Grid(n) => {
                let w = self.col_width(n);
                let y = if self.col == 0 && self.count > 0 {
                    self.cursor.y + self.row_h + self.spacing
                } else {
                    self.cursor.y
                };
                let x = self.region.min.x + self.col as f32 * (w + self.spacing);
                (Vec2::new(x, y), w)
            }
        }
    }

    /// Places an item. `fill` stretches it across the available width.
    #[inline(always)]
    pub fn place(&mut self, size: Vec2, fill: bool) -> Rect {
        let gap = if self.count > 0 { self.spacing } else { 0.0 };
        let r = match self.dir {
            Dir::Vertical => {
                self.cursor.y += gap;
                let w = if fill { self.region.width() } else { size.x };
                let r = Rect::from_xywh(self.region.min.x, self.cursor.y, w, size.y);
                self.cursor.y += size.y;
                r
            }
            Dir::Horizontal(Align::Start) | Dir::CenteredRow(Align::Start) => {
                self.cursor.x += gap;
                let w = if fill { (self.region.max.x - self.cursor.x).max(0.0) } else { size.x };
                let r = Rect::from_xywh(self.cursor.x, self.region.min.y, w, size.y);
                self.cursor.x += w;
                self.row_h = self.row_h.max(size.y);
                r
            }
            Dir::Horizontal(Align::End) | Dir::CenteredRow(Align::End) => {
                self.cursor.x -= gap;
                let w = if fill { (self.cursor.x - self.region.min.x).max(0.0) } else { size.x };
                self.cursor.x -= w;
                self.row_h = self.row_h.max(size.y);
                Rect::from_xywh(self.cursor.x, self.region.min.y, w, size.y)
            }
            Dir::Grid(n) => {
                let w = self.col_width(n);
                if self.col == 0 && self.count > 0 {
                    self.cursor.y += self.row_h + self.spacing;
                    self.row_h = 0.0;
                }
                let x = self.region.min.x + self.col as f32 * (w + self.spacing);
                let r = Rect::from_xywh(x, self.cursor.y, w, size.y);
                self.row_h = self.row_h.max(size.y);
                self.col = (self.col + 1) % n.max(1);
                r
            }
        };
        self.count += 1;
        let rel = r.max - self.region.min;
        self.extent = self.extent.max(rel);
        if let Dir::Horizontal(Align::End) | Dir::CenteredRow(Align::End) = self.dir {
            self.extent.x = self.region.max.x - self.cursor.x;
        }
        r
    }

    /// [`Layout::place`] for a centered row: places the item at the row's
    /// top like a plain row, then moves it to the explicit top edge or
    /// centers it within the row height. Kept out of line so other layouts
    /// pay nothing for it.
    #[inline(never)]
    fn place_centered(&mut self, size: Vec2, fill: bool) -> Rect {
        let r = self.place(size, fill);
        let top = self.region.min.y;
        let y = match &mut self.container {
            Container::CenteredRow { forced_y, height, .. } => match forced_y.take() {
                Some(y) => y,
                None => top + ((*height - size.y) * 0.5).max(0.0).round(),
            },
            _ => top,
        };
        let r = r.translate(Vec2::new(0.0, y - r.min.y));
        self.extent.y = self.extent.y.max(r.max.y - top);
        r
    }

    /// Measured content size.
    pub fn content_size(&self) -> Vec2 {
        self.extent
    }

    /// Advances the cursor by `amount` along the main axis.
    pub fn space(&mut self, amount: f32) {
        match self.dir {
            Dir::Vertical | Dir::Grid(_) => {
                self.cursor.y += amount;
                self.extent.y = self.extent.y.max(self.cursor.y - self.region.min.y);
            }
            Dir::Horizontal(Align::Start) | Dir::CenteredRow(Align::Start) => self.cursor.x += amount,
            Dir::Horizontal(Align::End) | Dir::CenteredRow(Align::End) => self.cursor.x -= amount,
        }
    }
}

impl Ui {
    #[inline(always)]
    pub(crate) fn top(&mut self) -> &mut Layout {
        self.layouts.last_mut().expect("root layout always present during a frame")
    }

    /// Allocates space for an item in the current container.
    #[inline]
    pub fn allocate(&mut self, mut size: Vec2, mut fill: bool) -> Rect {
        if let Some(w) = self.next_width.take() {
            size.x = w;
            fill = false;
        }
        let size = Vec2::new(extent(size.x), extent(size.y));
        let top = self.top();
        let r = if let Dir::CenteredRow(_) = top.dir { top.place_centered(size, fill) } else { top.place(size, fill) };
        self.last_item = r;
        r
    }

    /// [`Ui::allocate`] for an item that is skipped because it is outside
    /// the clip rectangle: in a centered row it stays at the row's top edge,
    /// since its position within the row is never seen.
    #[inline(always)]
    pub(crate) fn allocate_offscreen(&mut self, mut size: Vec2, mut fill: bool) -> Rect {
        if let Some(w) = self.next_width.take() {
            size.x = w;
            fill = false;
        }
        let r = self.top().place(Vec2::new(extent(size.x), extent(size.y)), fill);
        self.last_item = r;
        r
    }

    /// Overrides the width of the next allocated item (e.g. a text input or
    /// slider inside a row).
    pub fn set_next_item_width(&mut self, width: f32) {
        self.next_width = Some(extent(width));
    }

    /// Allocates an interactive region of `size` for a custom widget. The
    /// returned [`Response`] reports hover/press/click/focus like built-in
    /// widgets; paint it with [`Ui::painter`].
    pub fn allocate_interactive(&mut self, id_source: &str, size: Vec2) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        let id = self.make_id(id_source);
        let r = self.allocate(size, false);
        let resp = self.interact(id, r, Sense::BUTTON);
        self.record("custom", id_source, &resp);
        resp
    }

    /// Animated 0..1 hover/press/focus values for a custom widget response.
    pub fn interaction_anim(&mut self, resp: &Response) -> (f32, f32, f32) {
        let v = self.visual(resp);
        (v.hover, v.press, v.focus)
    }

    /// Draws the theme focus ring around `rect` with `t` in 0..=1.
    pub fn paint_focus_ring(&mut self, rect: Rect, radius: f32, t: f32) {
        self.focus_ring(rect, radius, t);
    }

    /// True when keyboard focus indication should be shown for `id`.
    pub fn wants_focus_ring(&self, id: Id) -> bool {
        self.focus_ring_visible(id)
    }

    /// Width available to the next item.
    pub fn available_width(&self) -> f32 {
        self.layouts.last().map(|l| l.peek().1).unwrap_or(0.0)
    }

    /// Adds empty space along the current layout direction.
    pub fn space(&mut self, amount: f32) {
        if self.frame_guard() && amount.is_finite() {
            self.top().space(amount.clamp(-MAX_EXTENT, MAX_EXTENT));
        }
    }

    /// Where the next child container starts and how wide it may be. In a
    /// centered row the child is offset by its height from the previous frame.
    #[inline]
    pub(crate) fn child_origin(&mut self) -> (Vec2, f32) {
        let top = self.top();
        let (origin, w) = top.peek();
        if let Dir::CenteredRow(_) = top.dir {
            return (self.centered_child_origin(origin), w);
        }
        (origin, w)
    }

    #[inline(never)]
    fn centered_child_origin(&mut self, mut origin: Vec2) -> Vec2 {
        let top = self.top();
        let Container::CenteredRow { id, height, .. } = top.container else { return origin };
        let i = top.count as usize;
        let h = self.states.get(&id).and_then(|s| s.downcast_ref::<RowHeights>()).and_then(|r| r.items.get(i));
        if let Some(&h) = h.filter(|h| **h > 0.0) {
            origin.y += ((height - h) * 0.5).max(0.0).round();
        }
        origin
    }

    /// Allocates a finished child container whose content starts at `top`.
    /// In a centered row it stays where its content was laid out, and its
    /// height is remembered for the next frame.
    #[inline]
    fn place_child(&mut self, size: Vec2, fill: bool, top: f32) -> Rect {
        if let Dir::CenteredRow(_) = self.top().dir {
            return self.place_centered_child(size, fill, top);
        }
        self.allocate(size, fill)
    }

    #[inline(never)]
    fn place_centered_child(&mut self, size: Vec2, fill: bool, top: f32) -> Rect {
        let mut row = None;
        if let Container::CenteredRow { id, forced_y, .. } = &mut self.top().container {
            *forced_y = Some(top);
            row = Some(*id);
        }
        let r = self.allocate(size, fill);
        if let Some(id) = row {
            let i = self.top().count as usize - 1;
            if let Some(h) = self.state_mut::<RowHeights>(id).items.get_mut(i) {
                *h = r.height();
            }
        }
        r
    }

    fn push_child(&mut self, dir: Dir, container: Container, width: Option<f32>) {
        let spacing = self.theme.metrics.spacing;
        let (origin, avail) = self.child_origin();
        let w = width.unwrap_or(avail);
        let region = Rect::from_min_size(origin, Vec2::new(w, f32::INFINITY));
        self.layouts.push(Layout::new(dir, region, spacing, container));
    }

    /// Starts a vertical group.
    pub fn begin_vertical(&mut self) {
        if self.frame_guard() {
            self.push_child(Dir::Vertical, Container::Plain, None);
        }
    }

    /// Starts a vertical group with a fixed width.
    pub fn begin_vertical_width(&mut self, width: f32) {
        if self.frame_guard() {
            self.push_child(Dir::Vertical, Container::FixedWidth, Some(extent(width)));
        }
    }

    /// Starts a horizontal row.
    pub fn begin_horizontal(&mut self, align: Align) {
        if self.frame_guard() {
            self.push_child(Dir::Horizontal(align), Container::Plain, None);
        }
    }

    /// Starts a row whose items are centered on the cross axis, e.g. a label
    /// next to a text input. The row uses its height from the previous frame
    /// (the control height in the first frame) and requests another frame
    /// when that height changes.
    pub fn begin_horizontal_centered(&mut self, align: Align) {
        if !self.frame_guard() {
            return;
        }
        let id = self.current_id().with("##centered-row").with_u64(self.centered_rows as u64);
        self.centered_rows += 1;
        let height = self
            .states
            .get(&id)
            .and_then(|s| s.downcast_ref::<RowHeights>())
            .and_then(|r| r.row)
            .unwrap_or(self.theme.metrics.control_height);
        self.push_child(Dir::CenteredRow(align), Container::CenteredRow { id, height, forced_y: None }, None);
    }

    /// Starts an equal-width column grid; items wrap after `columns`.
    pub fn begin_columns(&mut self, columns: u32) {
        if self.frame_guard() {
            let spacing = self.theme.metrics.spacing + 4.0;
            self.push_child(Dir::Grid(columns.max(1)), Container::Plain, None);
            self.top().spacing = spacing;
        }
    }

    /// Starts a card: a raised rounded surface whose size follows its content.
    pub fn begin_card(&mut self) {
        self.begin_card_ex(0.0, true);
    }

    /// Starts a card with a minimum height and optional elevation (shadow).
    pub fn begin_card_ex(&mut self, min_height: f32, elevated: bool) {
        if !self.frame_guard() {
            return;
        }
        let pad = self.theme.metrics.card_padding;
        let shadow = self.out.draw_list.reserve();
        let bg = self.out.draw_list.reserve();
        let (origin, w) = self.child_origin();
        let region =
            Rect::from_min_size(origin + Vec2::new(pad, pad), Vec2::new((w - pad * 2.0).max(0.0), f32::INFINITY));
        let spacing = self.theme.metrics.spacing;
        self.layouts.push(Layout::new(
            Dir::Vertical,
            region,
            spacing,
            Container::Card { shadow, bg, padding: pad, min_height, elevated, glass: None },
        ));
    }

    /// Starts a panel docked to the left edge of the remaining window area.
    pub fn begin_side_panel(&mut self, width: f32) {
        if !self.frame_guard() {
            return;
        }
        if self.layouts.len() != 1 {
            self.report(UiError::PanelNotAtRoot);
            self.begin_vertical();
            return;
        }
        let w = extent(width).min(self.remaining.width());
        let rect = Rect::from_min_size(self.remaining.min, Vec2::new(w, self.remaining.height()));
        self.remaining.min.x += w;
        let p = &self.theme.palette;
        self.out.draw_list.rect(rect, 0.0, p.panel, None);
        self.out.draw_list.rect(
            Rect::from_min_max(Vec2::new(rect.max.x - 1.0, rect.min.y), rect.max),
            0.0,
            p.border,
            None,
        );
        self.open_panel(rect);
    }

    /// Starts a bar docked to the top of the remaining window area.
    pub fn begin_top_bar(&mut self, height: f32) {
        if !self.frame_guard() {
            return;
        }
        if self.layouts.len() != 1 {
            self.report(UiError::PanelNotAtRoot);
            self.begin_horizontal(Align::Start);
            return;
        }
        let h = extent(height).min(self.remaining.height());
        let rect = Rect::from_min_size(self.remaining.min, Vec2::new(self.remaining.width(), h));
        self.remaining.min.y += h;
        let p = &self.theme.palette;
        self.out.draw_list.rect(
            Rect::from_min_max(Vec2::new(rect.min.x, rect.max.y - 1.0), rect.max),
            0.0,
            p.border,
            None,
        );
        self.open_panel(rect);
    }

    /// Starts the central panel filling the remaining window area.
    pub fn begin_central_panel(&mut self) {
        if !self.frame_guard() {
            return;
        }
        if self.layouts.len() != 1 {
            self.report(UiError::PanelNotAtRoot);
            self.begin_vertical();
            return;
        }
        let rect = self.remaining;
        self.remaining = Rect::from_min_size(rect.max, Vec2::ZERO);
        self.open_panel(rect);
    }

    fn open_panel(&mut self, rect: Rect) {
        let pad = self.theme.metrics.panel_padding;
        self.out.draw_list.push_clip(rect);
        let inner = rect.shrink2(pad, pad);
        let spacing = self.theme.metrics.spacing;
        self.layouts.push(Layout::new(Dir::Vertical, inner, spacing, Container::Panel));
    }

    /// Ends the innermost container started with any `begin_*` call.
    pub fn end(&mut self) {
        if !self.frame_guard() {
            return;
        }
        if self.layouts.len() <= 1 {
            self.report(UiError::NothingToEnd);
            return;
        }
        // Copy out what we need, then drop the layout in place.
        let n = self.layouts.len();
        let (container, content, child_dir, child_region_w, child_row_h, child_top) = {
            let c = &self.layouts[n - 1];
            (c.container, c.content_size(), c.dir, c.region.width(), c.row_h, c.region.min.y)
        };
        self.layouts.truncate(n - 1);
        match container {
            Container::Root => unreachable!("root is never popped"),
            Container::Panel => {
                self.out.draw_list.pop_clip();
            }
            Container::Plain | Container::FixedWidth => {
                let fill = matches!(
                    child_dir,
                    Dir::Vertical | Dir::Grid(_) | Dir::Horizontal(Align::End) | Dir::CenteredRow(Align::End)
                );
                // Inside a left-to-right row a group takes its content width so
                // following items sit next to it; elsewhere it fills.
                let in_row = self.top().dir.is_row_start();
                let fixed = matches!(container, Container::FixedWidth);
                let w = if fixed || (fill && !in_row) { child_region_w } else { content.x };
                let size = Vec2::new(w, content.y.max(child_row_h));
                let parent_vertical = matches!(self.top().dir, Dir::Vertical);
                self.place_child(size, parent_vertical && fill, child_top);
            }
            Container::CenteredRow { id, height, .. } => {
                let fill = matches!(child_dir, Dir::CenteredRow(Align::End));
                let in_row = self.top().dir.is_row_start();
                let w = if fill && !in_row { child_region_w } else { content.x };
                let size = Vec2::new(w, content.y.max(child_row_h));
                let parent_vertical = matches!(self.top().dir, Dir::Vertical);
                self.place_child(size, parent_vertical && fill, child_top);
                if (child_row_h - height).abs() > 0.5 {
                    self.request_repaint();
                }
                self.state_mut::<RowHeights>(id).row = Some(child_row_h);
            }
            Container::Transition { start, fade, offset } => {
                let in_row = self.top().dir.is_row_start();
                let w = if in_row { content.x } else { child_region_w };
                let size = Vec2::new(w, content.y.max(child_row_h) + offset.y.max(0.0));
                let parent_vertical = matches!(self.top().dir, Dir::Vertical);
                self.place_child(size, parent_vertical, child_top - offset.y);
                self.out.draw_list.fade_from(start, fade);
            }
            Container::Card { shadow, bg, padding, min_height, elevated, glass } => {
                let size = Vec2::new(child_region_w + padding * 2.0, (content.y + padding * 2.0).max(min_height));
                let parent_vertical = matches!(self.top().dir, Dir::Vertical);
                let r = self.place_child(size, parent_vertical, child_top - padding);
                if let Some(style) = glass {
                    let sh = DrawList::shadow_instance(
                        r.translate(Vec2::new(0.0, 8.0)),
                        style.radius,
                        18.0,
                        self.theme.palette.shadow.fade(0.6),
                    );
                    self.out.draw_list.fill_slot(shadow, sh);
                    self.out.draw_list.fill_slot(bg, DrawList::glass_instance(r, &style, 1.0));
                    return;
                }
                let (m, p) = (&self.theme.metrics, &self.theme.palette);
                let radius = m.radius_lg;
                let bw = m.border_width;
                if elevated {
                    let sh =
                        DrawList::shadow_instance(r.translate(Vec2::new(0.0, 4.0)), radius, 10.0, p.shadow.fade(0.55));
                    self.out.draw_list.fill_slot(shadow, sh);
                }
                let inst = DrawList::rect_instance(
                    r,
                    [radius; 4],
                    Fill::Vertical(p.surface_highlight, p.surface),
                    Some((bw, p.border)),
                );
                self.out.draw_list.fill_slot(bg, inst);
            }
            Container::Popup { id, shadow, bg, origin, width, pad, kind, start } => {
                self.end_floating(id, shadow, bg, origin, width, pad, kind, start, content);
            }
            Container::Region => self.out.draw_list.pop_clip(),
            Container::Collapse { id, open_t } => {
                self.out.draw_list.pop_clip();
                let full = content.y.max(0.0);
                self.state_mut::<CollapseHeight>(id).0 = full;
                let parent_vertical = matches!(self.top().dir, Dir::Vertical);
                self.place_child(Vec2::new(child_region_w, (full * open_t).round()), parent_vertical, child_top);
            }
            Container::Scroll { id, outer, offset } => {
                self.out.draw_list.pop_clip();
                let content_h = content.y;
                let max = (content_h - outer.height()).max(0.0);
                let depth =
                    self.layouts.iter().filter(|l| matches!(l.container, Container::Scroll { .. })).count() as u32;
                self.scroll_areas.push((id, outer.intersect(&self.out.draw_list.clip()), depth, max > 0.0));
                let parent_vertical = matches!(self.top().dir, Dir::Vertical);
                self.place_child(outer.size(), parent_vertical, outer.min.y);
                let offset = self.scroll_focus_into_view(id, outer, offset, max);
                self.scrollbar(id, outer, offset, content_h, max);
            }
        }
    }

    /// Left side panel.
    pub fn side_panel<R>(&mut self, width: f32, f: impl FnOnce(&mut Ui) -> R) -> R {
        self.begin_side_panel(width);
        let r = f(self);
        self.end();
        r
    }

    /// Top bar.
    pub fn top_bar<R>(&mut self, height: f32, f: impl FnOnce(&mut Ui) -> R) -> R {
        self.begin_top_bar(height);
        let r = f(self);
        self.end();
        r
    }

    /// Central panel.
    pub fn central_panel<R>(&mut self, f: impl FnOnce(&mut Ui) -> R) -> R {
        self.begin_central_panel();
        let r = f(self);
        self.end();
        r
    }

    /// Vertical group.
    pub fn vertical<R>(&mut self, f: impl FnOnce(&mut Ui) -> R) -> R {
        self.begin_vertical();
        let r = f(self);
        self.end();
        r
    }

    /// Horizontal row.
    pub fn horizontal<R>(&mut self, f: impl FnOnce(&mut Ui) -> R) -> R {
        self.begin_horizontal(Align::Start);
        let r = f(self);
        self.end();
        r
    }

    /// A row whose items are centered on the cross axis (e.g. a label next to a text input).
    pub fn horizontal_centered<R>(&mut self, f: impl FnOnce(&mut Ui) -> R) -> R {
        self.begin_horizontal_centered(Align::Start);
        let r = f(self);
        self.end();
        r
    }

    /// A form row: `label` in a column of `theme().metrics.label_width`
    /// (shortened with "…" when longer) and the widgets built by `f` to its
    /// right, centered on the row. Text after `##` is not shown.
    pub fn labeled<R>(&mut self, label: &str, f: impl FnOnce(&mut Ui) -> R) -> R {
        self.begin_horizontal_centered(Align::Start);
        let width = self.theme.metrics.label_width;
        self.set_next_item_width(width);
        self.label_truncated(rugi_core::id::split_label(label).0, crate::TextRole::Body, Some(width));
        let r = f(self);
        self.end();
        r
    }

    /// Right-aligned horizontal row (items flow right to left).
    pub fn horizontal_right<R>(&mut self, f: impl FnOnce(&mut Ui) -> R) -> R {
        self.begin_horizontal(Align::End);
        let r = f(self);
        self.end();
        r
    }

    /// Equal-width column grid.
    pub fn columns<R>(&mut self, columns: u32, f: impl FnOnce(&mut Ui) -> R) -> R {
        self.begin_columns(columns);
        let r = f(self);
        self.end();
        r
    }

    /// Card.
    pub fn card<R>(&mut self, f: impl FnOnce(&mut Ui) -> R) -> R {
        self.begin_card();
        let r = f(self);
        self.end();
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertical_places_with_spacing() {
        let mut l = Layout::new(Dir::Vertical, Rect::from_xywh(0.0, 0.0, 100.0, 500.0), 10.0, Container::Plain);
        let a = l.place(Vec2::new(50.0, 20.0), false);
        let b = l.place(Vec2::new(50.0, 20.0), true);
        assert_eq!(a, Rect::from_xywh(0.0, 0.0, 50.0, 20.0));
        assert_eq!(b, Rect::from_xywh(0.0, 30.0, 100.0, 20.0));
        assert_eq!(l.content_size(), Vec2::new(100.0, 50.0));
    }

    #[test]
    fn grid_wraps_rows() {
        let mut l = Layout::new(Dir::Grid(2), Rect::from_xywh(0.0, 0.0, 210.0, 500.0), 10.0, Container::Plain);
        let a = l.place(Vec2::new(0.0, 30.0), false);
        let b = l.place(Vec2::new(0.0, 40.0), false);
        let c = l.place(Vec2::new(0.0, 30.0), false);
        assert_eq!(a, Rect::from_xywh(0.0, 0.0, 100.0, 30.0));
        assert_eq!(b, Rect::from_xywh(110.0, 0.0, 100.0, 40.0));
        assert_eq!(c, Rect::from_xywh(0.0, 50.0, 100.0, 30.0));
        assert_eq!(l.content_size().y, 80.0);
    }

    #[test]
    fn right_aligned_row() {
        let mut l =
            Layout::new(Dir::Horizontal(Align::End), Rect::from_xywh(0.0, 0.0, 300.0, 40.0), 8.0, Container::Plain);
        let a = l.place(Vec2::new(50.0, 30.0), false);
        let b = l.place(Vec2::new(40.0, 30.0), false);
        assert_eq!(a.max.x, 300.0);
        assert_eq!(b.max.x, 242.0);
        assert_eq!(l.content_size().x, 98.0);
    }
}
