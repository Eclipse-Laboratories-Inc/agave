use clap::{Parser, Subcommand};
use solana_sdk::clock::UnixTimestamp;
use std::str::FromStr;

/// SolSnap CLI - Manage and re-execute Solana snapshots and blocks.
#[derive(Parser, Debug)]
#[command(version = "0.1.0", about = "Re-execute Eclipse blocks from snapshot")]
pub(crate) struct Cli {
    /// Top-level subcommands: snapshot, blocks, reexecute
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone)]
pub enum Network {
    EclipseMainnet,
    EclipseDevnet,
    EclipseTestnet,
    SolanaMainnet,
}

impl Network {
    pub fn url(&self) -> &str {
        match self {
            Self::EclipseMainnet => "https://mainnetbeta-rpc.eclipse.xyz/",
            Self::EclipseDevnet => "https://devnet.dev2.eclipsenetwork.xyz/",
            Self::EclipseTestnet => "https://testnet.dev2.eclipsenetwork.xyz/",
            Self::SolanaMainnet => todo!("No default public RPC for Solana mainnet"),
        }
    }

    pub fn creation_time(&self) -> UnixTimestamp {
        match self {
            Network::EclipseMainnet => 1722252148,
            Network::EclipseDevnet => {
                todo!()
            }
            Network::EclipseTestnet => 1712572914,
            Network::SolanaMainnet => {
                todo!()
            }
        }
    }
}

impl Default for Network {
    fn default() -> Self {
        Self::EclipseMainnet
    }
}

impl FromStr for Network {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "eclipse-mainnet" => Ok(Self::EclipseMainnet),
            "eclipse-devnet" => Ok(Self::EclipseDevnet),
            "eclipse-testnet" => Ok(Self::EclipseTestnet),
            "solana-mainnet" => Ok(Self::SolanaMainnet),
            _ => Err(format!("Unknown network: {}", s)),
        }
    }
}

// Implement `Display` so Clap can show the default in help text (via .to_string()).
impl std::fmt::Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EclipseMainnet => write!(f, "eclipse-mainnet"),
            Self::EclipseDevnet => write!(f, "eclipse-devnet"),
            Self::EclipseTestnet => write!(f, "eclipse-testnet"),
            Self::SolanaMainnet => write!(f, "solana-mainnet"),
        }
    }
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    /// Subcommands for listing and downloading Solana snapshots.
    Snapshot {
        #[command(subcommand)]
        action: SnapshotCommand,
    },
    /// Reexecute N blocks on top of a downloaded snapshot
    Replay {
        #[arg(long, default_value_t = Network::default())]
        network: Network,
        /// Number of blocks to reexecute
        #[arg(short, long, default_value_t = 1)]
        n_blocks: usize,
        /// The block to execute (if in cache already)
        #[arg(short, long)]
        snapshot_slot: Option<u64>,
    },
}

#[derive(Subcommand, Debug)]
pub(crate) enum SnapshotCommand {
    /// List the latest snapshots available.
    List {
        #[arg(long, default_value_t = Network::default())]
        network: Network,
    },
    /// Download a snapshot by ID.
    Download {
        #[arg(long, default_value = "eclipse-testnet")]
        network: Network,
    },
}
