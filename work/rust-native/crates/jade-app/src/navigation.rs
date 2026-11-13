//! Bounded navigation of lightweight view snapshots. Never stores application data.
use eframe::egui;
use std::collections::VecDeque;

pub const DEFAULT_HISTORY_LIMIT: usize = 64;

#[derive(Clone, Debug)]
pub struct Entry<T: Clone> {
    /// Canonical route for the view that was actually displayed.
    pub route: String,
    /// Human-readable destination, used by hints and the history menu.
    pub label: String,
    pub state: T,
}

#[derive(Debug)]
pub struct History<T: Clone> {
    back: VecDeque<Entry<T>>,
    forward: VecDeque<Entry<T>>,
    limit: usize,
}

impl<T: Clone> Default for History<T> {
    fn default() -> Self {
        Self::with_limit(DEFAULT_HISTORY_LIMIT)
    }
}

impl<T: Clone> History<T> {
    pub fn with_limit(limit: usize) -> Self {
        Self {
            back: VecDeque::new(),
            forward: VecDeque::new(),
            limit,
        }
    }

    /// Record the current view before navigating to a distinct canonical route.
    /// A no-op must preserve the forward history, just like clicking the current tab.
    pub fn record(&mut self, current: Entry<T>, target_route: &str) -> bool {
        if current.route == target_route {
            return false;
        }
        Self::push(&mut self.back, current, self.limit);
        self.forward.clear();
        true
    }

    pub fn back(&mut self, current: Entry<T>) -> Option<Entry<T>> {
        self.back_steps(current, 1)
    }
    pub fn forward(&mut self, current: Entry<T>) -> Option<Entry<T>> {
        self.forward_steps(current, 1)
    }

    /// Choosing menu item n is the same as pressing Back n times. Snapshots on
    /// the other side remain traversable in the opposite order with Forward.
    pub fn back_steps(&mut self, current: Entry<T>, steps: usize) -> Option<Entry<T>> {
        Self::travel(
            &mut self.back,
            &mut self.forward,
            current,
            steps,
            self.limit,
        )
    }
    pub fn forward_steps(&mut self, current: Entry<T>, steps: usize) -> Option<Entry<T>> {
        Self::travel(
            &mut self.forward,
            &mut self.back,
            current,
            steps,
            self.limit,
        )
    }

    pub fn peek_back(&self, current_route: &str) -> Option<&Entry<T>> {
        self.back
            .iter()
            .rev()
            .find(|entry| entry.route != current_route)
    }
    pub fn peek_forward(&self, current_route: &str) -> Option<&Entry<T>> {
        self.forward
            .iter()
            .rev()
            .find(|entry| entry.route != current_route)
    }

    /// Most-recent destination first. Index + 1 is the matching *_steps count.
    /// Only consecutive equivalent entries are skipped; visiting A -> B -> A is
    /// legitimate history and must not erase the earlier visit to A.
    pub fn back_targets(&self, current_route: &str, count: usize) -> Vec<&Entry<T>> {
        Self::targets(&self.back, current_route, count)
    }
    pub fn forward_targets(&self, current_route: &str, count: usize) -> Vec<&Entry<T>> {
        Self::targets(&self.forward, current_route, count)
    }

    /// Oldest first, allowing callers to find the most recent non-map source.
    pub fn back_entries(&self) -> impl DoubleEndedIterator<Item = &Entry<T>> + ExactSizeIterator {
        self.back.iter()
    }

    pub fn clear(&mut self) {
        self.back.clear();
        self.forward.clear();
    }

    fn push(queue: &mut VecDeque<Entry<T>>, entry: Entry<T>, limit: usize) {
        if limit == 0 {
            return;
        }
        if queue.back().is_some_and(|last| last.route == entry.route) {
            // Keep the latest filters/camera/input for a repeated current view.
            *queue.back_mut().unwrap() = entry;
        } else {
            queue.push_back(entry);
        }
        while queue.len() > limit {
            queue.pop_front();
        }
    }

    fn travel(
        from: &mut VecDeque<Entry<T>>,
        to: &mut VecDeque<Entry<T>>,
        mut current: Entry<T>,
        steps: usize,
        limit: usize,
    ) -> Option<Entry<T>> {
        let mut moved = false;
        for _ in 0..steps {
            // Legacy or direct route changes may leave an equivalent top entry.
            // Remove it instead of requiring a second press that looks like a no-op.
            while from
                .back()
                .is_some_and(|entry| entry.route == current.route)
            {
                from.pop_back();
            }
            let Some(target) = from.pop_back() else {
                break;
            };
            Self::push(to, current, limit);
            current = target;
            moved = true;
        }
        moved.then_some(current)
    }

    fn targets<'a>(
        queue: &'a VecDeque<Entry<T>>,
        current_route: &str,
        count: usize,
    ) -> Vec<&'a Entry<T>> {
        let mut result = Vec::with_capacity(count.min(queue.len()));
        let mut previous = current_route;
        for entry in queue.iter().rev() {
            if result.len() == count {
                break;
            }
            if entry.route != previous {
                result.push(entry);
                previous = &entry.route;
            }
        }
        result
    }
}

/// Only positions are kept in history. Egui's drag, momentum and animation state
/// belong to the current interaction and must never be replayed after Back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollBookmark {
    pub id: egui::Id,
    pub offset: egui::Vec2,
}

fn scroll_registry() -> egui::Id {
    egui::Id::new("jade-navigation-scroll-frame")
}

pub fn begin_scroll_frame(ctx: &egui::Context) {
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<Vec<egui::Id>>(scroll_registry())
            .clear()
    });
}

fn register_scroll(ctx: &egui::Context, id: egui::Id) {
    ctx.data_mut(|data| {
        let ids = data.get_temp_mut_or_default::<Vec<egui::Id>>(scroll_registry());
        if !ids.contains(&id) {
            ids.push(id);
        }
    });
}

pub fn capture_scrolls(ctx: &egui::Context) -> Vec<ScrollBookmark> {
    let ids = ctx
        .data(|data| data.get_temp::<Vec<egui::Id>>(scroll_registry()))
        .unwrap_or_default();
    ids.into_iter()
        .filter_map(|id| {
            egui::scroll_area::State::load(ctx, id).map(|state| ScrollBookmark {
                id,
                offset: state.offset,
            })
        })
        .collect()
}

pub fn restore_scrolls(ctx: &egui::Context, bookmarks: &[ScrollBookmark]) {
    for bookmark in bookmarks {
        if !bookmark.offset.x.is_finite() || !bookmark.offset.y.is_finite() {
            continue;
        }
        let mut state = egui::scroll_area::State::default();
        state.offset = bookmark.offset;
        state.store(ctx, bookmark.id);
    }
}

/// Compatibility wrapper: the application keeps its existing builders,
/// virtualized rows and return values while registering the actual persistent ID.
#[must_use]
pub struct ScrollArea(egui::ScrollArea);
impl ScrollArea {
    pub fn vertical() -> Self {
        Self(egui::ScrollArea::vertical())
    }
    pub fn horizontal() -> Self {
        Self(egui::ScrollArea::horizontal())
    }
    pub fn both() -> Self {
        Self(egui::ScrollArea::both())
    }
    pub fn new(direction: impl Into<egui::Vec2b>) -> Self {
        Self(egui::ScrollArea::new(direction))
    }
    pub fn id_salt(self, salt: impl std::hash::Hash) -> Self {
        Self(self.0.id_salt(salt))
    }
    pub fn max_height(self, height: f32) -> Self {
        Self(self.0.max_height(height))
    }
    pub fn max_width(self, width: f32) -> Self {
        Self(self.0.max_width(width))
    }
    pub fn min_scrolled_height(self, height: f32) -> Self {
        Self(self.0.min_scrolled_height(height))
    }
    pub fn min_scrolled_width(self, width: f32) -> Self {
        Self(self.0.min_scrolled_width(width))
    }
    pub fn auto_shrink(self, shrink: impl Into<egui::Vec2b>) -> Self {
        Self(self.0.auto_shrink(shrink))
    }
    pub fn scroll_bar_visibility(self, visibility: egui::scroll_area::ScrollBarVisibility) -> Self {
        Self(self.0.scroll_bar_visibility(visibility))
    }
    pub fn scroll_offset(self, offset: egui::Vec2) -> Self {
        Self(self.0.scroll_offset(offset))
    }
    pub fn vertical_scroll_offset(self, offset: f32) -> Self {
        Self(self.0.vertical_scroll_offset(offset))
    }
    pub fn horizontal_scroll_offset(self, offset: f32) -> Self {
        Self(self.0.horizontal_scroll_offset(offset))
    }
    pub fn animated(self, animated: bool) -> Self {
        Self(self.0.animated(animated))
    }
    pub fn scroll_source(self, source: egui::scroll_area::ScrollSource) -> Self {
        Self(self.0.scroll_source(source))
    }
    pub fn stick_to_bottom(self, stick: bool) -> Self {
        Self(self.0.stick_to_bottom(stick))
    }
    pub fn stick_to_right(self, stick: bool) -> Self {
        Self(self.0.stick_to_right(stick))
    }

    pub fn show<R>(
        self,
        ui: &mut egui::Ui,
        add: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::scroll_area::ScrollAreaOutput<R> {
        let output = self.0.show(ui, add);
        register_scroll(ui.ctx(), output.id);
        output
    }
    pub fn show_rows<R>(
        self,
        ui: &mut egui::Ui,
        height: f32,
        count: usize,
        add: impl FnOnce(&mut egui::Ui, std::ops::Range<usize>) -> R,
    ) -> egui::scroll_area::ScrollAreaOutput<R> {
        let output = self.0.show_rows(ui, height, count, add);
        register_scroll(ui.ctx(), output.id);
        output
    }
    pub fn show_viewport<R>(
        self,
        ui: &mut egui::Ui,
        add: impl FnOnce(&mut egui::Ui, egui::Rect) -> R,
    ) -> egui::scroll_area::ScrollAreaOutput<R> {
        let output = self.0.show_viewport(ui, add);
        register_scroll(ui.ctx(), output.id);
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(route: &str, state: u32) -> Entry<u32> {
        Entry {
            route: route.into(),
            label: format!("页面 {route}"),
            state,
        }
    }
    fn routes(history: &History<u32>, current: &str) -> Vec<String> {
        history
            .back_targets(current, 64)
            .into_iter()
            .map(|e| e.route.clone())
            .collect()
    }

    #[test]
    fn back_and_forward_restore_exact_states_in_visit_order() {
        let mut h = History::default();
        assert!(h.record(entry("#lookup/parents/A", 17), "#paldex/A"));
        assert!(h.record(entry("#paldex/A", 200), "#spawns/A"));
        let restored = h.back(entry("#spawns/A", 900)).unwrap();
        assert_eq!(
            (restored.route.as_str(), restored.state),
            ("#paldex/A", 200)
        );
        let restored = h.back(restored).unwrap();
        assert_eq!(
            (restored.route.as_str(), restored.state),
            ("#lookup/parents/A", 17)
        );
        let restored = h.forward(restored).unwrap();
        assert_eq!(
            (restored.route.as_str(), restored.state),
            ("#paldex/A", 200)
        );
        let restored = h.forward(restored).unwrap();
        assert_eq!(
            (restored.route.as_str(), restored.state),
            ("#spawns/A", 900)
        );
        assert!(h.peek_forward(&restored.route).is_none());
    }

    #[test]
    fn no_op_preserves_forward_but_a_new_branch_clears_it() {
        let mut h = History::default();
        h.record(entry("A", 1), "B");
        let restored = h.back(entry("B", 2)).unwrap();
        assert!(!h.record(restored.clone(), "A"));
        assert_eq!(h.peek_forward("A").unwrap().route, "B");
        assert!(h.record(restored, "C"));
        assert!(h.peek_forward("C").is_none());
        assert_eq!(h.peek_back("C").unwrap().route, "A");
    }

    #[test]
    fn adjacent_duplicates_keep_latest_snapshot_without_erasing_nonadjacent_visits() {
        let mut h = History::default();
        h.record(entry("A", 1), "B");
        // Repeated current-state record replaces the stale capture of A.
        h.record(entry("A", 5), "C");
        assert_eq!(h.back_entries().len(), 1);
        assert_eq!(h.peek_back("C").unwrap().state, 5);
        h.record(entry("C", 6), "A");
        h.record(entry("A", 7), "D");
        assert_eq!(routes(&h, "D"), ["A", "C", "A"]);
    }

    #[test]
    fn equivalent_current_entries_are_skipped_with_one_press_and_preview_matches() {
        let mut h = History::default();
        h.back = VecDeque::from([entry("A", 1), entry("B", 2), entry("B", 3), entry("C", 4)]);
        assert_eq!(routes(&h, "C"), ["B", "A"]);
        assert_eq!(h.peek_back("C").unwrap().state, 3);
        let restored = h.back(entry("C", 9)).unwrap();
        assert_eq!((restored.route.as_str(), restored.state), ("B", 3));
        let restored = h.back(restored).unwrap();
        assert_eq!((restored.route.as_str(), restored.state), ("A", 1));
        assert_eq!(
            h.forward_targets("A", 3)
                .iter()
                .map(|e| e.route.as_str())
                .collect::<Vec<_>>(),
            ["B", "C"]
        );
    }

    #[test]
    fn bounded_history_drops_oldest_and_menu_jump_retains_forward_order() {
        let mut h = History::with_limit(3);
        for i in 0..6 {
            h.record(entry(&i.to_string(), i), &(i + 1).to_string());
        }
        assert_eq!(routes(&h, "6"), ["5", "4", "3"]);
        assert_eq!(h.back_targets("6", 2).len(), 2);
        let restored = h.back_steps(entry("6", 6), 2).unwrap();
        assert_eq!((restored.route.as_str(), restored.state), ("4", 4));
        assert_eq!(
            h.forward_targets("4", 3)
                .iter()
                .map(|e| e.route.as_str())
                .collect::<Vec<_>>(),
            ["5", "6"]
        );
        let restored = h.forward_steps(restored, usize::MAX).unwrap();
        assert_eq!(restored.route, "6");
        assert!(h.forward(restored).is_none());
    }

    #[test]
    fn empty_zero_step_disabled_and_clear_are_safe() {
        let mut h = History::with_limit(0);
        assert!(h.record(entry("A", 1), "B"));
        assert!(h.peek_back("B").is_none());
        assert!(h.back(entry("B", 2)).is_none());
        let mut h = History::default();
        h.record(entry("A", 1), "B");
        assert!(h.back_steps(entry("B", 2), 0).is_none());
        assert_eq!(h.peek_back("B").unwrap().route, "A");
        h.back(entry("B", 2));
        h.clear();
        assert!(h.peek_forward("A").is_none());
        assert!(h.peek_back("A").is_none());
    }

    #[test]
    fn moving_between_stacks_never_clones_snapshot_payloads() {
        use std::{cell::Cell, rc::Rc};
        struct Counted(Rc<Cell<usize>>);
        impl Clone for Counted {
            fn clone(&self) -> Self {
                self.0.set(self.0.get() + 1);
                Self(self.0.clone())
            }
        }
        let clones = Rc::new(Cell::new(0));
        let make = |route: &str| Entry {
            route: route.into(),
            label: route.into(),
            state: Counted(clones.clone()),
        };
        let mut h = History::default();
        h.record(make("A"), "B");
        h.record(make("B"), "C");
        let restored = h.back_steps(make("C"), 2).unwrap();
        h.forward_steps(restored, 2).unwrap();
        assert_eq!(clones.get(), 0);
    }

    fn scroll_frame(
        ctx: &egui::Context,
        time: f64,
        page: &str,
        offset: Option<f32>,
        events: Vec<egui::Event>,
    ) -> egui::scroll_area::ScrollAreaOutput<()> {
        let mut result = None;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400., 300.),
                )),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ctx| {
                begin_scroll_frame(ctx);
                egui::CentralPanel::default().show(ctx, |ui| {
                    let mut scroll = ScrollArea::vertical()
                        .id_salt(page)
                        .max_height(120.)
                        .auto_shrink([false, false]);
                    if let Some(offset) = offset {
                        scroll = scroll.vertical_scroll_offset(offset);
                    }
                    result = Some(scroll.show(ui, |ui| ui.set_min_height(3000.)));
                });
            },
        );
        result.unwrap()
    }

    #[test]
    fn real_scroll_offsets_restore_per_view_without_retaining_other_page_ids() {
        let ctx = egui::Context::default();
        let a = scroll_frame(&ctx, 0., "A", Some(420.), vec![]);
        let bookmarks = capture_scrolls(&ctx);
        assert_eq!(
            bookmarks,
            vec![ScrollBookmark {
                id: a.id,
                offset: a.state.offset
            }]
        );
        let b = scroll_frame(&ctx, 0.02, "B", Some(30.), vec![]);
        assert_ne!(a.id, b.id);
        assert_eq!(
            capture_scrolls(&ctx)
                .iter()
                .map(|b| b.id)
                .collect::<Vec<_>>(),
            [b.id]
        );
        scroll_frame(&ctx, 0.04, "A", Some(0.), vec![]);
        restore_scrolls(&ctx, &bookmarks);
        let restored = scroll_frame(&ctx, 0.06, "A", None, vec![]);
        assert!(
            (restored.state.offset.y - 420.).abs() < 0.5,
            "{:?}",
            restored.state.offset
        );
    }

    #[test]
    fn restoring_a_real_drag_scroll_removes_momentum_before_the_next_frame() {
        let ctx = egui::Context::default();
        scroll_frame(&ctx, 0., "drag", Some(420.), vec![]);
        let start = egui::pos2(100., 100.);
        scroll_frame(
            &ctx,
            0.02,
            "drag",
            None,
            vec![egui::Event::PointerMoved(start)],
        );
        scroll_frame(
            &ctx,
            0.04,
            "drag",
            None,
            vec![egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            }],
        );
        scroll_frame(
            &ctx,
            0.06,
            "drag",
            None,
            vec![egui::Event::PointerMoved(egui::pos2(100., 70.))],
        );
        scroll_frame(
            &ctx,
            0.08,
            "drag",
            None,
            vec![egui::Event::PointerMoved(egui::pos2(100., 30.))],
        );
        let end = egui::pos2(100., 10.);
        let moving = scroll_frame(
            &ctx,
            0.10,
            "drag",
            None,
            vec![
                egui::Event::PointerMoved(end),
                egui::Event::PointerButton {
                    pos: end,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                },
            ],
        );
        assert!(
            moving.state.velocity().length() > 1.,
            "drag should produce momentum: {:?}",
            moving.state.velocity()
        );
        let bookmarks = capture_scrolls(&ctx);
        restore_scrolls(&ctx, &bookmarks);
        let restored = egui::scroll_area::State::load(&ctx, moving.id).unwrap();
        assert_eq!(restored.velocity(), egui::Vec2::ZERO);
        let next = scroll_frame(&ctx, 0.12, "drag", None, vec![]);
        assert!((next.state.offset.y - bookmarks[0].offset.y).abs() < 0.5);
    }
}
