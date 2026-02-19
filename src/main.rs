mod application;
mod domain;
mod infrastructure;
mod presentation;

use crate::application::orchestrator::Orchestrator;
use crate::domain::entities::Config;
use crate::domain::ports::{ConfigPort, MonitorPort};
use crate::infrastructure::config_loader::FileConfigLoader;
use crate::infrastructure::docker_monitor::DockerMonitor;
use crate::infrastructure::notification_adapter::WebhookNotifier;
use crate::infrastructure::remediation_adapter::SystemRemediator;
use crate::infrastructure::system_monitor::SystemMonitor; // Updated import
use crate::presentation::api::{start_server, AppState};
use clap::{Parser, Subcommand};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Webhook {
        #[command(subcommand)]
        action: WebhookCommands,
    },
}

#[derive(Subcommand, Debug)]
enum WebhookCommands {
    Add {
        #[arg(long)]
        url: String,
        #[arg(long)]
        provider: String,
    },
    List,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let args = Args::parse();

    // Handle CLI commands first
    if let Some(Commands::Webhook { action }) = &args.command {
        let loader = FileConfigLoader;
        match action {
            WebhookCommands::Add { url, provider } => {
                match loader.add_webhook(&args.config, url.clone(), provider.clone()) {
                    Ok(_) => info!("Successfully added webhook to {}", args.config),
                    Err(e) => error!("Failed to add webhook: {}", e),
                }
            }
            WebhookCommands::List => match loader.list_webhooks(&args.config) {
                Ok(hooks) => {
                    println!("Configured Webhooks:");
                    for (url, provider) in hooks {
                        println!("- Provider: {}, URL: {}", provider, url);
                    }
                }
                Err(e) => error!("Failed to list webhooks: {}", e),
            },
        }
        return Ok(());
    }

    info!("Starting Aegis-RS (Clean Architecture Edition)...");

    let config_loader = FileConfigLoader;
    let config = config_loader.load_config(&args.config).unwrap_or_else(|e| {
        error!("Failed to load config: {}. Using defaults.", e);
        Config::default()
    });
    info!("Configuration loaded.");

    let remediator = Arc::new(SystemRemediator::new(config.clone()));
    let notifier = Arc::new(WebhookNotifier::new(config.clone()));
    let docker_monitor = Arc::new(DockerMonitor::new());
    let system_monitor = Arc::new(SystemMonitor::new());

    // Fetch initial metrics to populate history
    let initial_metrics = system_monitor.get_system_metrics().await;

    let metrics = Arc::new(Mutex::new(initial_metrics.clone()));

    // Store history of SystemMetrics for graph generation
    let mut history_deque = VecDeque::with_capacity(60);
    for _ in 0..60 {
        history_deque.push_back(initial_metrics.clone());
    }
    let history = Arc::new(Mutex::new(history_deque));

    let orchestrator = Orchestrator::new(
        config.clone(),
        docker_monitor,
        system_monitor,
        remediator.clone(),
        notifier.clone(),
        metrics.clone(),
        history.clone(),
    );

    // Start Orchestrator in background
    let orchestrator_arc = Arc::new(orchestrator);
    let orch_clone = orchestrator_arc.clone();
    let orchestrator_handle = tokio::spawn(async move {
        orch_clone.run().await;
    });

    let app_state = AppState {
        metrics: metrics.clone(),
        history: history.clone(),
        secret_token: config.secret_token.clone(),
        api_port: config.api_port,
        config: Arc::new(Mutex::new(config)),
    };

    let server_handle = tokio::spawn(async move {
        start_server(app_state).await;
    });

    tokio::select! {
        _ = server_handle => error!("API server task matched"),
        _ = orchestrator_handle => error!("Orchestrator task matched"),
    }

    Ok(())
}
