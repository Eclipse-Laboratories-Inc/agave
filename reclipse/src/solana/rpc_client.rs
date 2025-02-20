use crate::toolkit::download_file::{download_file, FileDownloadError};
use indicatif::ProgressBar;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_response::RpcSnapshotSlotInfo;
use solana_transaction_status_client_types::{EncodedConfirmedBlock, UiTransactionEncoding};
use std::path::{Path, PathBuf};

/// An RPC client to get data from the Solana public API.
#[derive(thiserror::Error, Debug)]
pub enum SolanaRpcError {
    #[error("The node has no snapshot")]
    NoSnapshot,

    #[error(transparent)]
    ClientError(#[from] solana_client::client_error::ClientError),

    #[error(transparent)]
    DownloadError(#[from] FileDownloadError),
}

pub struct HighestSnapshotSlot {
    pub full: u64,
    pub incremental: u64,
}

pub struct SolanaRpcClient {
    /// Solana RPC client, used to access 99% of RPC methods.
    solana_rpc_client: RpcClient,
    /// An additional reqwest client, used to implement methods not supported
    /// in the solana-client crate.
    reqwest_client: reqwest::Client,
    /// Solana RPC URL.
    url: String,
}

impl SolanaRpcClient {
    pub fn new<U: ToString>(url: U) -> Self {
        let url_string = url.to_string();

        Self {
            solana_rpc_client: RpcClient::new(url_string.clone()),
            reqwest_client: reqwest::Client::new(),
            url: url_string,
        }
    }

    pub async fn get_highest_snapshot_slot(&self) -> Result<RpcSnapshotSlotInfo, SolanaRpcError> {
        let snapshot_slot_info = self.solana_rpc_client.get_highest_snapshot_slot().await?;
        Ok(snapshot_slot_info)
    }

    /// Downloads a full snapshot file to the directory `output_dir`.
    pub async fn download_full_snapshot(
        &self,
        output_dir: &Path,
    ) -> Result<PathBuf, SolanaRpcError> {
        let full_snapshot_url = format!("{}snapshot.tar.bz2", self.url);
        let progress_bar = ProgressBar::new(0);
        let downloaded_file_path = download_file(
            &self.reqwest_client,
            &full_snapshot_url,
            output_dir,
            &progress_bar,
        )
        .await
        .map_err(FileDownloadError::from)?;

        Ok(downloaded_file_path)
    }

    pub async fn download_block(
        &self,
        slot_height: u64,
    ) -> Result<EncodedConfirmedBlock, SolanaRpcError> {
        self.solana_rpc_client
            .get_block_with_encoding(slot_height, UiTransactionEncoding::Base58)
            .await
            .map_err(SolanaRpcError::from)
    }
}
