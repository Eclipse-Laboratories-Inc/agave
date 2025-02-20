use crate::cli::Network;
use crate::solana::rpc_client::SolanaRpcClient;
use solana_accounts_db::accounts_db::ACCOUNTS_DB_CONFIG_FOR_BENCHMARKS;
use solana_runtime::bank::Bank;
use solana_runtime::bank_forks::BankForks;
use solana_runtime::runtime_config::RuntimeConfig;
use solana_runtime::snapshot_archive_info::FullSnapshotArchiveInfo;
use solana_runtime::snapshot_bank_utils::bank_from_snapshot_archives;
use solana_sdk::clock::Slot;
use solana_sdk::genesis_config::create_genesis_config;
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status_client_types::EncodedConfirmedBlock;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn get_snapshot_from_cache(snapshot_dir: &Path, snapshot_slot: Slot) -> Option<PathBuf> {
    if !snapshot_dir.exists() {
        return None;
    }
    for dir_entry in std::fs::read_dir(snapshot_dir)
        .unwrap_or_else(|e| panic!("Could not list snapshot directory: {}", e))
    {
        let dir_entry = dir_entry.unwrap();
        if dir_entry.file_type().unwrap().is_dir() {
            continue;
        }
        if dir_entry
            .file_name()
            .to_string_lossy()
            .contains(&snapshot_slot.to_string())
        {
            return Some(dir_entry.path());
        }
    }
    None
}

pub async fn replay(
    network: Network,
    n_blocks: usize,
    snapshot_slot: Option<u64>,
) -> Result<(), anyhow::Error> {
    let rpc_client = SolanaRpcClient::new(network.url());
    let snapshot_slot_info = rpc_client.get_highest_snapshot_slot().await?;

    let snapshot_slot = match snapshot_slot {
        Some(x) => x,
        None => rpc_client.get_highest_snapshot_slot().await?.full,
    };

    let temp_dir = std::path::PathBuf::from("/tmp/reclipse_cache");
    let cache_dir = temp_dir.join("cache");
    let snapshot_dir = cache_dir.join("snapshots");

    let downloaded_snapshot_path = match get_snapshot_from_cache(&snapshot_dir, snapshot_slot) {
        Some(path) => {
            println!("Snapshot already downloaded ({})", path.to_string_lossy());
            path
        }
        None => {
            println!(
                "Downloading latest snapshot (slot {})...",
                snapshot_slot_info.full
            );
            let path = rpc_client.download_full_snapshot(&snapshot_dir).await?;
            println!("Download finished!");
            path
        }
    };

    let accounts_dir = cache_dir.join("accounts");
    let full_snapshot_archive = FullSnapshotArchiveInfo::new_from_path(downloaded_snapshot_path)?;

    let genesis_config = {
        let (mut genesis_config, _) = create_genesis_config(500);
        genesis_config.creation_time = network.creation_time();
        genesis_config
    };

    // Rebuild the bank and ensure it passes verification
    println!("Building bank from snapshot...");
    let (snapshot_bank, _) = bank_from_snapshot_archives(
        &[accounts_dir],
        &snapshot_dir,
        &full_snapshot_archive,
        None,
        &genesis_config,
        &RuntimeConfig::default(),
        None,
        None,
        None,
        false,
        false,
        false,
        false,
        Some(ACCOUNTS_DB_CONFIG_FOR_BENCHMARKS),
        None,
        Arc::new(AtomicBool::new(false)),
    )?;

    // TODO: can we do better than this mess?
    let bank_forks = BankForks::new_rw_arc(snapshot_bank);
    let snapshot_bank = bank_forks.read().unwrap().working_bank();

    // Create a new Bank that is a child of snapshot_bank, for slot X+1:
    let replay_bank = Arc::new(Bank::new_from_parent(
        snapshot_bank,
        &Pubkey::new_unique(), // arbitrary collector ID, usually the leader's node pubkey
        snapshot_slot + 1,
    ));

    println!("Successfully built bank from snapshot!");

    // Download the N blocks that follow the snapshot
    let first_slot = snapshot_slot + 1;
    let last_slot = first_slot + n_blocks as u64;
    let mut blocks = Vec::with_capacity(n_blocks);
    for slot_height in first_slot..last_slot {
        println!("Downloading block {slot_height}...");
        blocks.push(rpc_client.download_block(slot_height).await?);
    }

    for block in blocks {
        println!("Replaying slot #{}...", block.parent_slot + 1);
        let decoded_transactions: Vec<_> = block
            .transactions
            .iter()
            .map(|tx| {
                tx.transaction
                    .decode()
                    .unwrap_or_else(|| panic!("Failed to decode transaction"))
                    .into_legacy_transaction()
                    .unwrap_or_else(|| panic!("Failed to convert into legacy transaction"))
            })
            .collect();

        let results = replay_bank.process_transactions(decoded_transactions.iter());
        for (i, result) in results.into_iter().enumerate() {
            if let Err(e) = result {
                panic!("Failed to reexecute tx #{i}: {e}");
            }
        }
    }

    println!(
        "Successfully replayed slots [{} -> {}).",
        first_slot, last_slot
    );

    // let encoded_transaction = &blocks[0].transactions[0];
    // println!("Encoded transaction: {:?}", encoded_transaction);
    //
    // let transaction = encoded_transaction
    //     .transaction
    //     .decode()
    //     .expect("Failed to decode tx")
    //     .into_legacy_transaction()
    //     .expect("Failed to convert into legacy tx");
    // replay_bank.process_transaction(&transaction)?;

    Ok(())
}

pub async fn replay_from_test_vector(test_vector_path: PathBuf) -> Result<(), anyhow::Error> {
    // Read directory for files in `test_vector_path`
    let files = std::fs::read_dir(&test_vector_path)?;
    // Find file which starts with `snapshot-`
    let snapshot_file = files
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.file_name()?.to_string_lossy().starts_with("snapshot-") {
                Some(path.clone())
            } else {
                None
            }
        })
        .next()
        .ok_or_else(|| anyhow::anyhow!("No snapshot file found in test vector directory"))?;
    println!("Snapshot file: {:?}", snapshot_file);
    let snapshot_slot = snapshot_file
        .file_name()
        .unwrap()
        .to_string_lossy()
        .split('-')
        .skip(1)
        .next()
        .unwrap()
        .parse::<u64>()
        .unwrap();

    let accounts_dir = std::path::PathBuf::from("/tmp/reclipse_replay_cache/accounts");
    // Delete the accounts directory if it exists
    if accounts_dir.exists() {
        println!("Deleting temporary accounts directory...");
        std::fs::remove_dir_all(&accounts_dir)?;
    }
    let full_snapshot_archive = FullSnapshotArchiveInfo::new_from_path(snapshot_file.clone())?;

    let genesis_config = {
        let (mut genesis_config, _) = create_genesis_config(500);
        genesis_config.creation_time = Network::EclipseTestnet.creation_time();
        genesis_config
    };

    let snapshot_dir = test_vector_path.parent().unwrap();

    // Rebuild the bank and ensure it passes verification
    println!("Building bank from snapshot...");
    let (snapshot_bank, _) = bank_from_snapshot_archives(
        &[accounts_dir],
        &snapshot_dir,
        &full_snapshot_archive,
        None,
        &genesis_config,
        &RuntimeConfig::default(),
        None,
        None,
        None,
        false,
        false,
        false,
        false,
        Some(ACCOUNTS_DB_CONFIG_FOR_BENCHMARKS),
        None,
        Arc::new(AtomicBool::new(false)),
    )?;

    println!(
        "Built bank slot height = {}, bank_hash: {:?}",
        snapshot_bank.slot(),
        snapshot_bank.hash()
    );

    // TODO: can we do better than this mess?
    let bank_forks = BankForks::new_rw_arc(snapshot_bank);
    let snapshot_bank = bank_forks.read().unwrap().working_bank();

    // Create a new Bank that is a child of snapshot_bank, for slot X+1:
    let replay_bank = Arc::new(Bank::new_from_parent(
        snapshot_bank,
        &Pubkey::new_unique(), // arbitrary collector ID, usually the leader's node pubkey
        snapshot_slot + 1,
    ));

    println!("Successfully built bank from snapshot!");

    let files = std::fs::read_dir(&test_vector_path)?;
    // Filter the block files of the name `block_plus_*.bin`
    let mut block_files = files
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path
                .file_name()?
                .to_string_lossy()
                .starts_with("block_plus_")
            {
                Some(path)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    block_files.sort();

    println!("Block files: {:#?}", block_files);

    for block_file in block_files {
        println!("Replaying block file: {:?}", block_file);
        let file_bytes = std::fs::read(&block_file)?;
        let block: EncodedConfirmedBlock = serde_json::from_slice(&file_bytes)?;
        println!(
            "Block parent slot: {:?}, parent_hash: {:?}, blockhash: {:?}, num_transactions: {}",
            block.parent_slot,
            block.previous_blockhash,
            block.blockhash,
            block.transactions.len()
        );
    }

    Ok(())
}
