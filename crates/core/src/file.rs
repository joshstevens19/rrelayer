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
