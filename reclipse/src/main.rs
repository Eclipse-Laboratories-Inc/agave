use clap::Parser;

pub mod cli;
pub mod commands;
pub mod solana;
pub mod toolkit;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let is_download_phase = std::env::var("RECLIPSE_DOWNLOAD_PHASE")
        .unwrap()
        .parse::<bool>()
        .unwrap();
    println!("RECLIPSE_DOWNLOAD_PHASE: {}", is_download_phase);

    if is_download_phase {
        commands::snapshot::download_test_vectors(cli::Network::EclipseTestnet, 2, 2)
            .await
            .unwrap();
    } else {
        let env = std::env::var("RECLIPSE_TESTVECTORS").unwrap();
        println!("RECLIPSE_TESTVECTORS: {}", env);
        commands::replay::replay_from_test_vector(std::path::PathBuf::from(env)).await?;
    }

    Ok(())
}
