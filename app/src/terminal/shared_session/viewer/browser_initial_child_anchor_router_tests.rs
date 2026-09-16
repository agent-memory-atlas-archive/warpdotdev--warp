use std::sync::Arc;

use chrono::Utc;
use parking_lot::Mutex;
use warpui::{App, ModelHandle, ViewHandle};

use super::*;
use crate::ai::ambient_agents::{AmbientAgentTask, AmbientAgentTaskState};
use crate::ai::blocklist::BlocklistAIHistoryModel;
use crate::features::FeatureFlag;
use crate::test_util::add_window_with_terminal;
use crate::test_util::terminal::initialize_app_for_terminal_view;

const PARENT_TASK_ID: &str = "11111111-1111-1111-1111-111111111111";
const CHILD_TASK_ID: &str = "22222222-2222-2222-2222-222222222222";
const OTHER_PARENT_TASK_ID: &str = "33333333-3333-3333-3333-333333333333";

#[test]
fn restores_a_seeded_child_when_discovery_registers_it() {
    let _unified_stack = FeatureFlag::OrchestrationUnifiedStack.override_enabled(true);
    App::test((), |mut app| async move {
        let (terminal_view, viewer_model, router) =
            setup(&mut app, ChildAnchor::Selected(task_id(CHILD_TASK_ID)));
        let restored = observe_restorations(&mut app, &terminal_view);

        router.update(&mut app, |router, ctx| {
            router.initial_anchor_fetch_in_flight = true;
            router.handle_streamer_event(
                &OrchestrationEventStreamerEvent::ViewerModeSeeded {
                    parent_task_id: task_id(PARENT_TASK_ID),
                    child_run_ids: vec![task_id(CHILD_TASK_ID)],
                },
                ctx,
            );
        });
        viewer_model.update(&mut app, |model, ctx| {
            model.register_child(child_task(), ctx);
        });

        let child_conversation_id = viewer_model.read(&app, |model, _| {
            model.registered_children()[&task_id(CHILD_TASK_ID)]
        });
        assert_eq!(*restored.lock(), vec![Some(child_conversation_id)]);
    });
}

#[test]
fn clears_an_invalid_anchor_after_the_seed_settles() {
    App::test((), |mut app| async move {
        let (terminal_view, _, router) = setup(&mut app, ChildAnchor::Invalid);
        let restored = observe_restorations(&mut app, &terminal_view);

        router.update(&mut app, |router, ctx| {
            router.handle_streamer_event(
                &OrchestrationEventStreamerEvent::ViewerModeSeeded {
                    parent_task_id: task_id(PARENT_TASK_ID),
                    child_run_ids: vec![],
                },
                ctx,
            );
        });

        assert_eq!(*restored.lock(), vec![None]);
    });
}

#[test]
fn ignores_hydration_for_another_parent() {
    App::test((), |mut app| async move {
        let (terminal_view, _, router) = setup(&mut app, ChildAnchor::Invalid);
        let restored = observe_restorations(&mut app, &terminal_view);

        router.update(&mut app, |router, ctx| {
            router.handle_streamer_event(
                &OrchestrationEventStreamerEvent::ViewerModeSeeded {
                    parent_task_id: task_id(OTHER_PARENT_TASK_ID),
                    child_run_ids: vec![],
                },
                ctx,
            );
        });

        router.read(&app, |router, _| {
            assert!(router.seeded_child_ids.is_none());
            assert!(!router.initial_anchor_resolution_emitted);
        });
        assert!(restored.lock().is_empty());
    });
}

fn setup(
    app: &mut App,
    initial_child_anchor: ChildAnchor,
) -> (
    ViewHandle<TerminalView>,
    ModelHandle<OrchestrationViewerModel>,
    ModelHandle<BrowserInitialChildAnchorRouter>,
) {
    initialize_app_for_terminal_view(app);
    let terminal_view = add_window_with_terminal(app, None);
    let terminal_view_id = terminal_view.id();
    BlocklistAIHistoryModel::handle(app).update(app, |history, ctx| {
        let conversation_id =
            history.start_new_conversation(terminal_view_id, false, true, false, ctx);
        history.set_viewing_shared_session_for_conversation(conversation_id, true);
        history.set_active_conversation_id(conversation_id, terminal_view_id, ctx);
    });
    let viewer_model = app.add_model(|ctx| {
        OrchestrationViewerModel::new(
            task_id(PARENT_TASK_ID),
            terminal_view_id,
            terminal_view.downgrade(),
            ctx,
        )
    });
    let router = app.add_model(|ctx| {
        BrowserInitialChildAnchorRouter::new_with_anchor(
            task_id(PARENT_TASK_ID),
            terminal_view.downgrade(),
            viewer_model.clone(),
            initial_child_anchor,
            ctx,
        )
    });
    (terminal_view, viewer_model, router)
}

fn observe_restorations(
    app: &mut App,
    terminal_view: &ViewHandle<TerminalView>,
) -> Arc<Mutex<Vec<Option<AIConversationId>>>> {
    let restored = Arc::new(Mutex::new(vec![]));
    let events = restored.clone();
    app.update(|ctx| {
        ctx.subscribe_to_view(terminal_view, move |_, event, _| {
            if let TerminalViewEvent::RestoreInitialChildAnchor { conversation_id } = event {
                events.lock().push(*conversation_id);
            }
        });
    });
    restored
}

fn task_id(id: &str) -> AmbientAgentTaskId {
    id.parse().expect("hardcoded task id parses")
}

fn child_task() -> AmbientAgentTask {
    let now = Utc::now();
    AmbientAgentTask {
        task_id: task_id(CHILD_TASK_ID),
        parent_run_id: Some(PARENT_TASK_ID.to_string()),
        title: "Worker".to_string(),
        state: AmbientAgentTaskState::Queued,
        prompt: String::new(),
        created_at: now,
        started_at: Some(now),
        updated_at: now,
        run_time: Some("PT1S".parse().unwrap()),
        status_message: None,
        source: None,
        execution_location: None,
        session_id: None,
        session_link: None,
        creator: None,
        executor: None,
        conversation_id: None,
        request_usage: None,
        is_sandbox_running: false,
        agent_config_snapshot: None,
        artifacts: vec![],
        last_event_sequence: None,
        children: vec![],
        debug_agent_available: false,
        scope: None,
    }
}
