use bzip2::read::BzDecoder;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};

use indicatif::ProgressBar;
use zstd::stream::read::Decoder;

use std::path::{Path, PathBuf};
use tar::Archive;
use tempfile::NamedTempFile;

#[derive(thiserror::Error, Debug)]
pub enum FileDecompressionError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("Unsupported archive type: {extension}")]
    UnsupportedArchiveType { extension: String },

    #[error("Could not determine archive type")]
    UnknownArchiveType,
}

/// Strips a .tar.bz2, .tar.zst or any other double extension from the filename.
pub fn strip_tar_extension(path: PathBuf) -> PathBuf {
    path.with_extension("").with_extension("")
}

fn decompress_file_zstd(
    input_path: &Path,
    output_path: &Path,
    progress_bar: &ProgressBar,
) -> Result<(), FileDecompressionError> {
    // Open the input file
    let input_file = File::open(input_path)?;
    let metadata = input_file.metadata()?;
    let total_size = metadata.len();

    // Set up the progress bar
    progress_bar.set_length(total_size);

    // Create a Zstandard decoder
    let mut decoder = Decoder::new(BufReader::new(input_file))?;
    let mut output_file = BufWriter::new(File::create(output_path)?);

    // Buffer for streaming decompression
    let mut buffer = vec![0; 8192]; // 8KB buffer
    let mut total_bytes_read = 0;

    // Decompress the file in chunks
    loop {
        let bytes_read = decoder.read(&mut buffer)?;
        if bytes_read == 0 {
            break; // End of file
        }
        output_file.write_all(&buffer[..bytes_read])?;
        total_bytes_read += bytes_read as u64;

        // Update the progress bar
        progress_bar.set_position(total_bytes_read);
    }

    // Finish the progress bar
    progress_bar.finish_with_message("Decompression complete");

    Ok(())
}

fn decompress_file_bz2(
    input_path: &Path,
    output_path: &Path,
    progress_bar: &ProgressBar,
) -> Result<(), FileDecompressionError> {
    let input_file = File::open(input_path)?;
    let mut output_file = File::create(output_path)?;

    let mut decoder = BzDecoder::new(BufReader::new(input_file));
    let mut buffer = vec![0; 8192]; // 8KB buffer

    loop {
        let bytes_read = decoder.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        output_file.write_all(&buffer[..bytes_read])?;
        progress_bar.inc(bytes_read as u64);
    }

    Ok(())
}

fn unpack_tar_file(
    tar_path: &Path,
    output_dir: &Path,
    progress_bar: &ProgressBar,
) -> Result<(), FileDecompressionError> {
    let tar_file = File::open(tar_path)?;
    let mut archive = Archive::new(BufReader::new(tar_file));

    // Estimate the number of entries for progress bar
    let entries = archive.entries()?.count();
    progress_bar.set_length(entries as u64);

    // Reopen the tar file to extract entries
    let tar_file = File::open(tar_path)?;
    let mut archive = Archive::new(BufReader::new(tar_file));
    let mut extracted = 0;

    for entry in archive.entries()? {
        let mut entry = entry?;
        entry.unpack_in(output_dir)?;
        extracted += 1;
        progress_bar.set_position(extracted as u64);
    }

    Ok(())
}

pub fn decompress_tar_archive(
    input_path: &Path,
    output_path: &Path,
) -> Result<(), FileDecompressionError> {
    // TODO: currently we decompress to a temporary file before unpacking the archive, use
    //       a buffer between the two steps instead.
    let tmp_tar_file = NamedTempFile::new()?;

    println!("Decompressing snapshot...");
    let decompression_progress_bar = ProgressBar::new_spinner();
    match input_path.extension() {
        Some(extension) => match extension.to_string_lossy().as_ref() {
            "bz2" => {
                decompress_file_bz2(input_path, tmp_tar_file.path(), &decompression_progress_bar)
            }
            "zst" => {
                decompress_file_zstd(input_path, tmp_tar_file.path(), &decompression_progress_bar)
            }
            other => {
                return Err(FileDecompressionError::UnsupportedArchiveType {
                    extension: other.to_string(),
                })
            }
        },
        None => return Err(FileDecompressionError::UnknownArchiveType),
    }?;
    decompression_progress_bar.finish_with_message("Decompression complete");

    println!("Unpacking archive...");
    let unpacking_progress_bar = ProgressBar::new(0);
    if !output_path.exists() {
        std::fs::create_dir_all(output_path)?;
    }
    unpack_tar_file(tmp_tar_file.path(), output_path, &unpacking_progress_bar)?;
    unpacking_progress_bar.finish_with_message("Unpacking complete");
    println!("Snapshot decompressed!");

    Ok(())
}
