use cc_converter::{anvil2cc, Anvil2CCConfig};
use phf::phf_map;
use slint::{invoke_from_event_loop, SharedString, VecModel};
use std::{
    path::PathBuf,
    sync::{atomic::Ordering, Arc},
};

slint::include_modules!();

enum ConverterRegistry {
    Anvil2CC,
}

static DEFAULT_CONVERTER: &str = "Anvil -> CubicChunks";
static VALID_CONVERTERS: phf::Map<&'static str, ConverterRegistry> = phf_map! {
    "Anvil -> CubicChunks" => ConverterRegistry::Anvil2CC,
};

fn open_folder(title: &str) -> Option<SharedString> {
    if let Some(path) = rfd::FileDialog::new().set_title(title).pick_file() {
        return Some(path.to_string_lossy().to_string().into());
    }
    None
}

fn set_converter_state(
    ui: MainWindow,
    (tasks_current, tasks_max): (i32, i32),
    (convert_current, convert_max): (i32, i32),
    (write_current, write_max): (i32, i32),
) {
    ui.set_tasks_queue_percentage((tasks_current as f32 / tasks_max as f32) * 100f32);
    ui.set_tasks_queue_text(format!("Tasks Queue: {}/{}", tasks_current, tasks_max).into());
    ui.set_convert_queue_percentage((convert_current as f32 / convert_max as f32) * 100f32);
    ui.set_convert_queue_text(format!("Convert Queue: {}/{}", convert_current, convert_max).into());
    ui.set_write_queue_percentage((write_current as f32 / write_max as f32) * 100f32);
    ui.set_write_queue_text(format!("Write Queue: {}/{}", write_current, write_max).into());
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = Arc::new(MainWindow::new()?);

    ui.on_open_folder(move |title: SharedString, old_path: SharedString| {
        let new_path = open_folder(title.as_str());
        if let Some(path) = new_path {
            return path;
        }
        old_path
    });

    let ui_ = ui.clone();
    ui.on_convert(move || {
        let ui_weak = ui_.as_weak();

        let src_path: String = ui_.invoke_get_src_path2().into();
        let src_path = PathBuf::from(src_path);

        let dst_path: String = ui_.invoke_get_dst_path2().into();
        let dst_path = PathBuf::from(dst_path);

        std::thread::spawn(move || {
            println!("{}", src_path.display());

            let waiter = anvil2cc(
                &src_path,
                &dst_path,
                Anvil2CCConfig {
                    fix_missing_tile_entities: true,
                },
            )
            .unwrap();

            let waiter = Arc::new(waiter);

            while !waiter.is_finished() {
                let ui = ui_weak.clone();
                let waiter = waiter.clone();
                invoke_from_event_loop(move || {
                    let ui = ui.unwrap();

                    let val = waiter.tasks_sent.load(Ordering::Relaxed) as i32;
                    set_converter_state(
                        ui,
                        (val, val), // TODO: actually count the tasks for the maximum
                        (waiter.convert_queue_fill.load(Ordering::Relaxed) as i32, 64),
                        (waiter.write_queue_fill.load(Ordering::Relaxed) as i32, 64),
                    );
                })
                .unwrap();
            }

            // Run once after all threads are finished to update the UI with the final values
            invoke_from_event_loop(move || {
                set_converter_state(
                    ui_weak.unwrap(),
                    (
                        waiter.tasks_sent.load(Ordering::Relaxed) as i32,
                        waiter.tasks_sent.load(Ordering::Relaxed) as i32,
                    ),
                    (waiter.convert_queue_fill.load(Ordering::Relaxed) as i32, 64),
                    (waiter.write_queue_fill.load(Ordering::Relaxed) as i32, 64),
                );
            })
            .unwrap();
        });
    });

    let converters: Vec<_> = VALID_CONVERTERS
        .keys()
        .map(|string| SharedString::from(*string))
        .collect();

    ui.invoke_set_converter_options(
        SharedString::from(DEFAULT_CONVERTER),
        VecModel::from_slice(&converters),
    );

    ui.show()?;
    slint::run_event_loop()?;
    ui.hide()?;
    Ok(())
}
