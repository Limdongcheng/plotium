use super::*;

#[test]
#[cfg(unix)]
fn commands_that_activate_tools_toggle_back_to_their_rest_tool() {
    crate::test_settings::isolated(
        "ui::commands::tests::settings_tests::commands_that_activate_tools_toggle_back_to_their_rest_tool",
        || {
            let catalog_app = app_with_nmr();
            let command_ids: Vec<_> = catalog(&catalog_app)
                .into_iter()
                // Plain actions may open native dialogs or external URLs; the checked
                // contract is what identifies catalog toggles without enumerating tools.
                .filter(|command| command.checked.is_some())
                .map(|command| command.id)
                .collect();

            for id in command_ids {
                let mut app = app_with_nmr();
                let before = app.session.tool;
                let ctx = egui::Context::default();
                let mut clipboard = crate::ui::clipboard_table::ClipboardTablePaste::default();

                execute(id, &mut app, &mut clipboard, &ctx);
                let activated = app.session.tool;
                if activated == before {
                    continue;
                }

                execute(id, &mut app, &mut clipboard, &ctx);
                assert_eq!(
                    app.session.tool,
                    activated.rest(),
                    "command {id:?} activated {activated:?} but did not toggle to its rest tool"
                );
            }
        },
    );
}
