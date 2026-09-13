use warp_core::ui::appearance::Appearance;
use warp_server_client::auth::AgentIdentity;
use warpui::App;
use warpui::platform::WindowStyle;

use super::{ApiKeyType, CreateApiKeyModal, CreateApiKeyModalAction};
use crate::auth::AuthStateProvider;
use crate::server::telemetry::context_provider::AppTelemetryContextProvider;
use crate::settings_view::keybindings::KeybindingChangedNotifier;
use crate::test_util::settings::initialize_settings_for_tests;
use crate::vim_registers::VimRegisters;
use crate::workspace::sync_inputs::SyncedInputState;
use crate::workspaces::user_workspaces::UserWorkspaces;

fn agent(uid: &str, name: &str, available: bool) -> AgentIdentity {
    AgentIdentity {
        uid: uid.to_string(),
        name: name.to_string(),
        available,
    }
}

#[test]
fn test_agent_dropdown_is_searchable() {
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);
        app.add_singleton_model(|_| AuthStateProvider::new_for_test());
        app.add_singleton_model(AppTelemetryContextProvider::new_context_provider);
        app.add_singleton_model(|_| Appearance::mock());
        app.add_singleton_model(|_| SyncedInputState::mock());
        app.add_singleton_model(|_| VimRegisters::new());
        app.add_singleton_model(|_| KeybindingChangedNotifier::mock());
        app.add_singleton_model(UserWorkspaces::default_mock);

        let (_, view) = app.add_window(WindowStyle::NotStealFocus, CreateApiKeyModal::new);

        view.update(&mut app, |modal, ctx| {
            modal.set_agents_for_test(
                vec![
                    agent("1", "Default Service Account", true),
                    agent("2", "Ben's Agent", true),
                    agent("3", "Server Migration Agent", true),
                    agent("4", "Unavailable Agent", false),
                ],
                ctx,
            );
        });

        let total = view.read(&app, |modal, ctx| modal.agent_dropdown.as_ref(ctx).len());
        assert_eq!(total, 3, "only available agents should be listed");
        let all_visible = view.read(&app, |modal, ctx| {
            modal
                .agent_dropdown
                .as_ref(ctx)
                .visible_items_len_for_test(ctx)
        });
        assert_eq!(all_visible, 3);

        view.update(&mut app, |modal, ctx| {
            modal.agent_dropdown.update(ctx, |dropdown, ctx| {
                dropdown.set_filter_query_for_test("BEN", ctx)
            });
        });
        let filtered = view.read(&app, |modal, ctx| {
            modal
                .agent_dropdown
                .as_ref(ctx)
                .visible_items_len_for_test(ctx)
        });
        assert_eq!(filtered, 1, "query should match only \"Ben's Agent\"");

        view.update(&mut app, |modal, ctx| {
            modal.agent_dropdown.update(ctx, |dropdown, ctx| {
                dropdown.set_filter_query_for_test("zzz", ctx)
            });
        });
        let none = view.read(&app, |modal, ctx| {
            modal
                .agent_dropdown
                .as_ref(ctx)
                .visible_items_len_for_test(ctx)
        });
        assert_eq!(none, 0);

        view.update(&mut app, |modal, ctx| {
            modal.agent_dropdown.update(ctx, |dropdown, ctx| {
                dropdown.set_filter_query_for_test("", ctx)
            });
        });
        let restored = view.read(&app, |modal, ctx| {
            modal
                .agent_dropdown
                .as_ref(ctx)
                .visible_items_len_for_test(ctx)
        });
        assert_eq!(restored, 3);
    })
}

#[test]
fn displayed_default_agent_enables_agent_key_creation() {
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);
        app.add_singleton_model(|_| AuthStateProvider::new_for_test());
        app.add_singleton_model(AppTelemetryContextProvider::new_context_provider);
        app.add_singleton_model(|_| Appearance::mock());
        app.add_singleton_model(|_| SyncedInputState::mock());
        app.add_singleton_model(|_| VimRegisters::new());
        app.add_singleton_model(|_| KeybindingChangedNotifier::mock());
        app.add_singleton_model(UserWorkspaces::default_mock);

        let (_, view) = app.add_window(WindowStyle::NotStealFocus, CreateApiKeyModal::new);

        view.update(&mut app, |modal, ctx| {
            modal.set_agents_for_test(
                vec![
                    agent("unavailable", "Unavailable Agent", false),
                    agent("default", "Default Service Account", true),
                ],
                ctx,
            );
        });

        view.read(&app, |modal, ctx| {
            assert_eq!(
                modal.agent_dropdown.as_ref(ctx).selected_item_label(),
                Some("Default Service Account".to_string())
            );
            assert_eq!(modal.selected_agent_uid(ctx).as_deref(), Some("default"));
            assert!(!modal.is_create_disabled(ApiKeyType::Agent, ctx));
        });
    })
}

#[test]
fn explicit_agent_selection_survives_agent_list_refresh() {
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);
        app.add_singleton_model(|_| AuthStateProvider::new_for_test());
        app.add_singleton_model(AppTelemetryContextProvider::new_context_provider);
        app.add_singleton_model(|_| Appearance::mock());
        app.add_singleton_model(|_| SyncedInputState::mock());
        app.add_singleton_model(|_| VimRegisters::new());
        app.add_singleton_model(|_| KeybindingChangedNotifier::mock());
        app.add_singleton_model(UserWorkspaces::default_mock);

        let (_, view) = app.add_window(WindowStyle::NotStealFocus, CreateApiKeyModal::new);

        view.update(&mut app, |modal, ctx| {
            modal.set_agents_for_test(
                vec![
                    agent("1", "Default Service Account", true),
                    agent("2", "Ben's Agent", true),
                ],
                ctx,
            );
            modal.agent_dropdown.update(ctx, |dropdown, ctx| {
                dropdown.set_selected_by_action(
                    CreateApiKeyModalAction::SelectAgent("2".to_string()),
                    ctx,
                );
            });
            modal.set_agents_for_test(
                vec![
                    agent("1", "Default Service Account", true),
                    agent("2", "Ben's Renamed Agent", true),
                ],
                ctx,
            );
        });

        view.read(&app, |modal, ctx| {
            assert_eq!(modal.selected_agent_uid(ctx).as_deref(), Some("2"));
            assert_eq!(
                modal.agent_dropdown.as_ref(ctx).selected_item_label(),
                Some("Ben's Renamed Agent".to_string())
            );
            assert!(!modal.is_create_disabled(ApiKeyType::Agent, ctx));
        });
    })
}

#[test]
fn agent_key_creation_stays_disabled_for_loading_or_empty_agent_lists() {
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);
        app.add_singleton_model(|_| AuthStateProvider::new_for_test());
        app.add_singleton_model(AppTelemetryContextProvider::new_context_provider);
        app.add_singleton_model(|_| Appearance::mock());
        app.add_singleton_model(|_| SyncedInputState::mock());
        app.add_singleton_model(|_| VimRegisters::new());
        app.add_singleton_model(|_| KeybindingChangedNotifier::mock());
        app.add_singleton_model(UserWorkspaces::default_mock);

        let (_, view) = app.add_window(WindowStyle::NotStealFocus, CreateApiKeyModal::new);

        view.read(&app, |modal, ctx| {
            assert!(modal.is_create_disabled(ApiKeyType::Agent, ctx));
            assert!(!modal.is_create_disabled(ApiKeyType::Personal, ctx));
        });

        view.update(&mut app, |modal, ctx| {
            modal.set_agents_for_test(vec![agent("1", "Default Service Account", true)], ctx);
            modal.is_loading_agents = true;
        });

        view.read(&app, |modal, ctx| {
            assert!(modal.is_create_disabled(ApiKeyType::Agent, ctx));
            assert!(!modal.is_create_disabled(ApiKeyType::Personal, ctx));
        });
    })
}
