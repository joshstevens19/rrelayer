use std::{fs, fs::File, io::Write, path::Path};

#[derive(thiserror::Error, Debug)]
pub enum WriteFileError {
    #[error("Could not create dir: {0}")]
    CouldNotCreateDir(std::io::Error),

    #[error("Could not convert string to bytes: {0}")]
    CouldNotConvertToBytes(std::io::Error),

    #[error("Could not create the file: {0}")]
    CouldNotCreateFile(std::io::Error),
}

/// Writes content to a file, creating parent directories as needed.
///
/// This function will:
/// 1. Create all parent directories if they don't exist
/// 2. Clean the content by trimming trailing whitespace from each line
/// 3. Write the cleaned content to the specified file
///
/// The content cleaning ensures consistent file formatting by removing
/// trailing whitespace that might cause issues in configuration files.
///
/// # Arguments
/// * `path` - The file path to write to
/// * `contents` - The string content to write
///
/// # Returns
/// * `Ok(())` - If the file was written successfully
/// * `Err(WriteFileError)` - If directory creation or file writing fails
///
/// # Example
/// ```rust,no_run
/// use std::path::Path;
/// use rrelayer_core::write_file;
///
/// let content = "Hello, world!\n  \nTrailing spaces will be removed.  ";
/// write_file(Path::new("output/example.txt"), content)?;
/// # Ok::<(), rrelayer_core::WriteFileError>(())
/// ```
pub fn write_file(path: &Path, contents: &str) -> Result<(), WriteFileError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(WriteFileError::CouldNotCreateDir)?;
    }

    let cleaned_contents: String =
        contents.lines().map(|line| line.trim_end()).collect::<Vec<&str>>().join("\n");

    let mut file = File::create(path).map_err(WriteFileError::CouldNotCreateFile)?;
    file.write_all(cleaned_contents.as_bytes()).map_err(WriteFileError::CouldNotConvertToBytes)?;
    Ok(())
}
