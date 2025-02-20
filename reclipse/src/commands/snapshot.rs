use crate::cli::Network;
use crate::solana::rpc_client::{SolanaRpcClient, SolanaRpcError};
use crate::toolkit::decompress_file::{decompress_tar_archive, strip_tar_extension};

pub async fn list_snapshots(network: Network) -> Result<(), SolanaRpcError> {
    println!("Listing the latest snapshots...");
    let rpc_client = SolanaRpcClient::new(network.url());
    let snapshot_slot_info = rpc_client.get_highest_snapshot_slot().await?;

    println!("Last snapshots:");
    println!("\tFull: {}", snapshot_slot_info.full);
    if let Some(incremental_snapshot_slot) = snapshot_slot_info.incremental {
        println!("\tIncremental: {}", incremental_snapshot_slot);
    }

    Ok(())
}

pub async fn download_full_snapshot(network: Network) -> Result<(), anyhow::Error> {
    let rpc_client = SolanaRpcClient::new(network.url());
    let snapshot_slot_info = rpc_client.get_highest_snapshot_slot().await?;

    println!(
        "Downloading latest snapshot (slot {})...",
        snapshot_slot_info.full
    );

    let temp_dir = std::path::PathBuf::from("/tmp/reclipse_cache");
    let cache_dir = temp_dir.join("cache");
    let snapshot_dir = cache_dir.join("snapshots");

    let _downloaded_snapshot = rpc_client.download_full_snapshot(&snapshot_dir).await?;
    println!("Download finished!");

    //let decompressed_snapshot_path = strip_tar_extension(downloaded_snapshot.clone());
    //decompress_tar_archive(&downloaded_snapshot, &decompressed_snapshot_path)?;

    Ok(())
}

pub async fn download_test_vectors(
    network: Network,
    n_loop: usize,
    n_blocks: usize,
) -> Result<(), anyhow::Error> {
    println!(
        "Downloading test vectors for network {:?}, n_loop = {}, n_blocks = {}",
        network, n_loop, n_blocks
    );
    let rpc_client = SolanaRpcClient::new(network.url());
    let mut latest_snapshot: Option<u64> = None;
    let cache_dir = std::path::PathBuf::from("/tmp/reclipse_cache");

    let mut i = 0;
    while i < n_loop {
        let current_snapshot_height = rpc_client.get_highest_snapshot_slot().await?.full;
        if let Some(latest_snapshot) = latest_snapshot {
            if current_snapshot_height == latest_snapshot {
                println!("No new snapshot available, sleeping for 30 seconds...");
                std::thread::sleep(std::time::Duration::from_secs(30));
                continue;
            }
        }
        println!(
            "Downloading snapshot for i = {} and slot = {}",
            i, current_snapshot_height
        );
        let snapshots_i_dir = cache_dir.join(format!("snapshots_{}", i));
        println!("    > Directory: {}", snapshots_i_dir.display());
        let _downloaded_snapshot = rpc_client.download_full_snapshot(&snapshots_i_dir).await?;

        let mut j = 1;
        while j <= n_blocks {
            let block = rpc_client
                .download_block(current_snapshot_height + j as u64)
                .await;
            if let Err(e) = block {
                println!("Failed to download block due to {:?} retrying...", e);
                std::thread::sleep(std::time::Duration::from_secs(5));
                continue;
            }
            let block = block.unwrap();
            assert_eq!(block.parent_slot, current_snapshot_height + j as u64 - 1);
            let serialized_block_json = serde_json::to_string(&block).unwrap();
            let block_path = snapshots_i_dir.join(format!("block_plus_{}.json", j));
            std::fs::write(&block_path, serialized_block_json).unwrap();
            println!("Downloaded block (plus {}): parent slot: {:?}, blockhash: {:?}, num_transactions: {}", j, block.parent_slot, block.blockhash, block.transactions.len());
            j += 1;
        }

        latest_snapshot = Some(current_snapshot_height);
        i += 1;
    }
    Ok(())
}
