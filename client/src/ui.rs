use crate::PROJECT_TITLE;
use crate::config::Config;
use crate::errors::ClientError;
use crate::ui::creator::AppCreator;
use thiserror::Error;

pub struct Ui {
    min_width: f32,
    min_height: f32,
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            min_width: 950.0,
            min_height: 550.0,
        }
    }
}

impl Ui {
    pub fn start(self, config: Config) -> Result<(), ClientError> {
        let native_options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_app_id(PROJECT_TITLE) // Wayland requirement
                .with_title(PROJECT_TITLE)
                .with_inner_size([self.min_width, self.min_height])
                .with_min_inner_size([self.min_width, self.min_height])
                .with_icon(
                    eframe::icon_data::from_png_bytes(
                        &include_bytes!("../assets/icon.png")[..],
                    ).map_err(|_| GraphicsBackendError::AppIcon)?
                ),
            centered: true,
            ..Default::default()
        };

        eframe::run_native(
            PROJECT_TITLE,
            native_options,
            Box::new(|cc| Ok(Box::new(AppCreator::new(cc, config)))),
        )
        .map_err(|error| GraphicsBackendError::FailedRunNative(error.to_string()))
        .map_err(ClientError::GraphicsBackend)
    }

    pub fn native_panic_message(error: ClientError) {
        rfd::MessageDialog::new()
            .set_title("Critical Error")
            .set_description(error.to_string())
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}

#[derive(Debug, Error)]
pub enum GraphicsBackendError {
    #[error("Failed to load app icon.")]
    AppIcon,

    #[error("Failed to run native app. {0}")]
    FailedRunNative(String),
}

pub mod creator;
pub mod modals;
pub mod workspace;
