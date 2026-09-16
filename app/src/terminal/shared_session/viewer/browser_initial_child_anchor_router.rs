use std::collections::{HashMap, HashSet};

use warpui::{Entity, ModelContext, ModelHandle, SingletonEntity, WeakViewHandle};

use super::orchestration_viewer_model::{OrchestrationViewerModel, OrchestrationViewerModelEvent};
use crate::ai::agent::conversation::AIConversationId;
use crate::ai::ambient_agents::AmbientAgentTaskId;
use crate::ai::blocklist::orchestration_event_streamer::{
    OrchestrationEventStreamer, OrchestrationEventStreamerEvent,
};
use crate::server::server_api::ServerApiProvider;
use crate::terminal::{Event as TerminalViewEvent, TerminalView};
#[cfg(target_family = "wasm")]
use crate::uri::browser_url_handler::parse_current_url;
#[cfg(target_family = "wasm")]
use crate::uri::viewer_location::ViewerLocation;
use crate::uri::viewer_location::{
    ChildAnchor, HydratedAnchorAction, hydrated_anchor_action, is_expected_direct_child,
};

pub(super) struct BrowserInitialChildAnchorRouter {
    parent_task_id: AmbientAgentTaskId,
    terminal_view: WeakViewHandle<TerminalView>,
    orchestration_viewer_model: ModelHandle<OrchestrationViewerModel>,
    initial_child_anchor: ChildAnchor,
    seeded_child_ids: Option<HashSet<AmbientAgentTaskId>>,
    registered_children: HashMap<AmbientAgentTaskId, AIConversationId>,
    initial_anchor_resolution_emitted: bool,
    initial_anchor_fetch_in_flight: bool,
}

impl Entity for BrowserInitialChildAnchorRouter {
    type Event = ();
}

impl BrowserInitialChildAnchorRouter {
    #[cfg(target_family = "wasm")]
    pub(super) fn new(
        parent_task_id: AmbientAgentTaskId,
        terminal_view: WeakViewHandle<TerminalView>,
        orchestration_viewer_model: ModelHandle<OrchestrationViewerModel>,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        let initial_child_anchor = parse_current_url()
            .as_ref()
            .and_then(ViewerLocation::parse)
            .map(|location| location.child_anchor)
            .unwrap_or(ChildAnchor::Root);
        Self::new_with_anchor(
            parent_task_id,
            terminal_view,
            orchestration_viewer_model,
            initial_child_anchor,
            ctx,
        )
    }

    fn new_with_anchor(
        parent_task_id: AmbientAgentTaskId,
        terminal_view: WeakViewHandle<TerminalView>,
        orchestration_viewer_model: ModelHandle<OrchestrationViewerModel>,
        initial_child_anchor: ChildAnchor,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        ctx.subscribe_to_model(&orchestration_viewer_model, |router, _, event, ctx| {
            router.handle_viewer_model_event(event, ctx);
        });
        ctx.subscribe_to_model(
            &OrchestrationEventStreamer::handle(ctx),
            |router, _, event, ctx| {
                router.handle_streamer_event(event, ctx);
            },
        );
        let registered_children = orchestration_viewer_model.as_ref(ctx).registered_children();

        Self {
            parent_task_id,
            terminal_view,
            orchestration_viewer_model,
            initial_child_anchor,
            seeded_child_ids: None,
            registered_children,
            initial_anchor_resolution_emitted: false,
            initial_anchor_fetch_in_flight: false,
        }
    }

    fn handle_viewer_model_event(
        &mut self,
        event: &OrchestrationViewerModelEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        match event {
            OrchestrationViewerModelEvent::ChildRegistered {
                task_id,
                conversation_id,
            } => {
                self.registered_children.insert(*task_id, *conversation_id);
                self.maybe_resolve_initial_child_anchor(ctx);
            }
        }
    }

    fn handle_streamer_event(
        &mut self,
        event: &OrchestrationEventStreamerEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        if let OrchestrationEventStreamerEvent::ViewerModeSeeded {
            parent_task_id,
            child_run_ids,
        } = event
            && *parent_task_id == self.parent_task_id
        {
            self.seeded_child_ids = Some(child_run_ids.iter().copied().collect());
            self.maybe_resolve_initial_child_anchor(ctx);
        }
    }

    fn maybe_resolve_initial_child_anchor(&mut self, ctx: &mut ModelContext<Self>) {
        if self.initial_anchor_resolution_emitted {
            return;
        }
        let Some(seeded_child_ids) = self.seeded_child_ids.as_ref() else {
            return;
        };
        let registered_child_ids = self.registered_children.keys().copied().collect();
        let conversation_id = match hydrated_anchor_action(
            self.initial_child_anchor,
            seeded_child_ids,
            &registered_child_ids,
        ) {
            HydratedAnchorAction::None => {
                self.initial_anchor_resolution_emitted = true;
                return;
            }
            HydratedAnchorAction::Wait => {
                let ChildAnchor::Selected(task_id) = self.initial_child_anchor else {
                    return;
                };
                self.fetch_initial_anchor_task(task_id, false, ctx);
                return;
            }
            HydratedAnchorAction::FetchAndVerify(task_id) => {
                self.fetch_initial_anchor_task(task_id, true, ctx);
                return;
            }
            HydratedAnchorAction::Clear => None,
            HydratedAnchorAction::Select(task_id) => Some(self.registered_children[&task_id]),
        };
        self.finish_initial_anchor_resolution(conversation_id, ctx);
    }

    fn fetch_initial_anchor_task(
        &mut self,
        task_id: AmbientAgentTaskId,
        verify_parent: bool,
        ctx: &mut ModelContext<Self>,
    ) {
        if self.initial_anchor_fetch_in_flight {
            return;
        }
        self.initial_anchor_fetch_in_flight = true;
        let ai_client = ServerApiProvider::as_ref(ctx).get_ai_client();
        let parent_task_id = self.parent_task_id;
        let orchestration_viewer_model = self.orchestration_viewer_model.clone();
        ctx.spawn(
            async move { ai_client.get_ambient_agent_task(&task_id).await },
            move |router, result, ctx| {
                router.initial_anchor_fetch_in_flight = false;
                match result {
                    Ok(task)
                        if task.task_id == task_id
                            && (!verify_parent
                                || is_expected_direct_child(&task, task_id, parent_task_id)) =>
                    {
                        orchestration_viewer_model.update(ctx, |model, ctx| {
                            model.register_child(task, ctx);
                        });
                    }
                    Ok(_) | Err(_) => {
                        router.finish_initial_anchor_resolution(None, ctx);
                    }
                }
            },
        );
    }

    fn finish_initial_anchor_resolution(
        &mut self,
        conversation_id: Option<AIConversationId>,
        ctx: &mut ModelContext<Self>,
    ) {
        self.initial_anchor_resolution_emitted = true;
        if let Some(view) = self.terminal_view.upgrade(ctx) {
            view.update(ctx, |_view, ctx| {
                ctx.emit(TerminalViewEvent::RestoreInitialChildAnchor { conversation_id });
            });
        }
    }
}

#[cfg(test)]
#[path = "browser_initial_child_anchor_router_tests.rs"]
mod tests;
