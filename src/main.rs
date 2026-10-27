use std::io;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    let cancel = CancellationToken::new();
    let signal = cancel.clone();
    let handler = tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            signal.cancel();
        }
    });
    let status = pge_cost_cli::entry(
        std::env::args_os(),
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
        cli_command_kit::Mode::from_env(),
        cancel,
    )
    .await;
    handler.abort();
    std::process::exit(i32::from(status));
}
