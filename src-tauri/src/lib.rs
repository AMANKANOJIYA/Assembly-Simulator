mod asim;
mod commands;
mod memory;
mod plugin;
mod simulator;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            commands::assemble,
            commands::assemble_check,
            commands::reset,
            commands::reset_with_program,
            commands::reset_for_arch_change,
            commands::step_forward,
            commands::step_forward_with_input,
            commands::step_back,
            commands::run_tick,
            commands::set_running,
            commands::get_state,
            commands::set_memory_size,
            commands::set_breakpoints,
            commands::load_program,
            commands::get_ui_schema,
            commands::get_register_schema,
            commands::write_asim_file,
            commands::read_asim_file,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
