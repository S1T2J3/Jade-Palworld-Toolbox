use crate::{
    assistant_basic,
    assistant_modules::ModuleManager,
    assistant_search::{SearchAction, SearchBox},
    assistant_view,
    data::{Data, text},
    store::Store,
};
use eframe::egui;
use std::path::PathBuf;

pub(crate) struct AssistantHost {
    modules: ModuleManager,
    resources: PathBuf,
    entry: SearchBox,
    world_id: Option<String>,
    basic: assistant_basic::AssistantView,
    kernel: Option<(PathBuf, assistant_view::AssistantView)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_explicit_prefill_survives_entering_its_own_world() {
        let profile = crate::store::tests::Temp::new();
        let mut host = AssistantHost::new(profile.0.clone(), PathBuf::new());
        host.prefill("first", "蛋糕怎么获得？".into());
        host.select_world("first");
        assert!(!host.entry.is_empty());
        host.select_world("second");
        assert!(host.entry.is_empty());
        host.prefill("second", "阿努比斯是什么属性？".into());
        host.reset_session();
        host.select_world("second");
        assert!(host.entry.is_empty());
    }
}

impl AssistantHost {
    pub(crate) fn new(profile: PathBuf, resources: PathBuf) -> Self {
        Self {
            modules: ModuleManager::new(profile),
            resources,
            entry: SearchBox::default(),
            world_id: None,
            basic: Default::default(),
            kernel: None,
        }
    }

    pub(crate) fn enhanced(&self) -> bool {
        self.modules.active_path().is_some()
    }

    pub(crate) fn has_dialog(&self) -> bool {
        self.kernel
            .as_ref()
            .is_some_and(|(_, view)| view.has_dialog())
    }

    pub(crate) fn dismiss_dialog(&mut self) -> bool {
        self.kernel
            .as_mut()
            .is_some_and(|(_, view)| view.dismiss_dialog())
    }

    pub(crate) fn reset_session(&mut self) {
        self.entry.reset();
        self.basic.reset_session();
        if let Some((_, view)) = &mut self.kernel {
            view.reset_session();
        }
    }

    fn select_world(&mut self, id: &str) {
        if self.world_id.as_deref() != Some(id) {
            self.reset_session();
            self.world_id = Some(id.to_owned());
        }
    }

    pub(crate) fn poll_hidden(&mut self) {
        if self.kernel.is_some() {
            self.modules.poll_deactivation();
            if !self.enhanced() {
                self.kernel = None;
            }
        }
    }

    fn sync_kernel(&mut self) {
        if self.modules.active_path() == self.kernel.as_ref().map(|(old, _)| old.as_path()) {
            return;
        }
        self.reset_session();
        self.kernel = self.modules.active_path().map(|path| {
            let view = assistant_view::AssistantView::new(path.to_owned(), self.resources.clone());
            (path.to_owned(), view)
        });
    }

    pub(crate) fn prefill(&mut self, world_id: &str, question: String) {
        self.select_world(world_id);
        self.sync_kernel();
        self.entry.prefill(question);
    }

    pub(crate) fn module_settings(&mut self, ui: &mut egui::Ui) {
        self.modules.ui(ui);
        self.sync_kernel();
    }

    pub(crate) fn ui(
        &mut self,
        ui: &mut egui::Ui,
        data: &Data,
        store: &Store,
        catalog: &crate::catalog::Catalog,
    ) -> Option<String> {
        let world_id = text(store.world(), "id");
        self.select_world(world_id);
        self.modules.poll();
        self.sync_kernel();
        let can_ask = if let Some((_, view)) = &mut self.kernel {
            view.prepare(world_id, ui.ctx());
            view.can_ask()
        } else {
            true
        };
        let submitted = match self.entry.ui_with_catalog(ui, data, can_ask, Some(catalog)) {
            Some(SearchAction::Open(route)) => return Some(route),
            Some(SearchAction::Ask(question)) => Some(question),
            None => None,
        };
        if let Some((_, view)) = &mut self.kernel {
            view.ui(ui, data, store, submitted, &mut self.entry)
        } else {
            self.basic.ui(ui, data, store, submitted, &mut self.entry)
        }
    }
}
