use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() -> eframe::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "babble=debug,info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Babble voice assistant");

    // Run the GUI application
    babble::ui::run()
}
