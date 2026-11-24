use crate::components::controls::build_compact_dropdown;
use crate::components::rows::build_action_row;
use crate::state::AppState;
use adw::prelude::*;
use genubar_core::PatchErrorPolicy;
use genubar_patcher::ConflictStrategy;
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

pub fn build_policy_group(
    state: &AppState,
    strategy_state: &Rc<RefCell<ConflictStrategy>>,
) -> adw::PreferencesGroup {
    let policy_group = adw::PreferencesGroup::builder()
        .title("Patch Engine Policies")
        .description("Configure rollback behavior and conflict handling when applying patches")
        .build();

    let cur_pol = state.config.borrow().settings.general.patch_error_policy;
    let err_row = build_action_row(
        "Error Handling Policy",
        "Strict rolls back on any error; Ignore I/O skips missing optional files",
    );
    let err_drop = build_compact_dropdown(
        &["Strict", "Ignore I/O", "Ignore All"],
        match cur_pol {
            PatchErrorPolicy::Strict => 0,
            PatchErrorPolicy::IgnoreSystemErrors => 1,
            PatchErrorPolicy::IgnoreAll => 2,
        },
    );
    {
        let st = state.clone();
        err_drop.connect_selected_notify(move |dd| {
            let pol = match dd.selected() {
                1 => PatchErrorPolicy::IgnoreSystemErrors,
                2 => PatchErrorPolicy::IgnoreAll,
                _ => PatchErrorPolicy::Strict,
            };
            st.config.borrow_mut().settings.general.patch_error_policy = pol;
            st.save_and_reload();
        });
    }
    err_row.add_suffix(&err_drop);
    policy_group.add(&err_row);

    let conf_row = build_action_row(
        "Conflict Resolution Strategy",
        "Action taken when another applied patch already owns the same config key",
    );
    let conf_drop = build_compact_dropdown(
        &["Overwrite", "Skip Step", "Disable Old Patch", "Abort"],
        match *strategy_state.borrow() {
            ConflictStrategy::Overwrite => 0,
            ConflictStrategy::SkipConflictingSteps => 1,
            ConflictStrategy::DisableConflictingPatch => 2,
            ConflictStrategy::Abort => 3,
        },
    );
    {
        let st_strat = strategy_state.clone();
        conf_drop.connect_selected_notify(move |dd| {
            *st_strat.borrow_mut() = match dd.selected() {
                1 => ConflictStrategy::SkipConflictingSteps,
                2 => ConflictStrategy::DisableConflictingPatch,
                3 => ConflictStrategy::Abort,
                _ => ConflictStrategy::Overwrite,
            };
        });
    }
    conf_row.add_suffix(&conf_drop);
    policy_group.add(&conf_row);

    policy_group
}
