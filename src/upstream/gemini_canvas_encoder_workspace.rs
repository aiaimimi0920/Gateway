use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::OwnedSemaphorePermit;

pub(super) const MAX_SOURCE_BYTES: usize = 32 * 1024 * 1024;
const MAX_ENCODED_BYTES: usize = 8 * 1024 * 1024;

pub(super) struct EncoderWorkspace {
    directory: PathBuf,
    pub(super) source: PathBuf,
    pub(super) output: PathBuf,
    _lease: Arc<OwnedSemaphorePermit>,
}

impl EncoderWorkspace {
    pub(super) fn create(
        source: &[u8],
        extension: &str,
        lease: Arc<OwnedSemaphorePermit>,
    ) -> io::Result<Self> {
        Self::create_in(
            &std::env::temp_dir().canonicalize()?,
            source,
            extension,
            lease,
        )
    }

    fn create_in(
        parent: &Path,
        source: &[u8],
        extension: &str,
        lease: Arc<OwnedSemaphorePermit>,
    ) -> io::Result<Self> {
        if source.len() > MAX_SOURCE_BYTES
            || !matches!(extension, "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp")
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid encoder source",
            ));
        }
        let directory = parent.join(format!("gateway-image-encoder-{}", uuid::Uuid::new_v4()));
        let mut builder = fs::DirBuilder::new();
        builder.recursive(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&directory)?;
        let workspace = Self {
            source: directory.join(format!("source.{extension}")),
            output: directory.join("encoded.jpg"),
            directory,
            _lease: lease,
        };
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        // Keep the guard alive until the file handle has closed, including write failures.
        let result = (|| options.open(&workspace.source)?.write_all(source))();
        result?;
        Ok(workspace)
    }

    pub(super) fn read_output(&self) -> io::Result<Vec<u8>> {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.custom_flags(
                windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT,
            );
        }
        let file = options.open(&self.output)?;
        Self::read_file(file)
    }

    fn read_file(file: File) -> io::Result<Vec<u8>> {
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.len() > MAX_ENCODED_BYTES as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid encoder output",
            ));
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes()
                & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
                != 0
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "encoder output is a reparse point",
                ));
            }
        }
        let mut bytes = Vec::new();
        file.take(MAX_ENCODED_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.is_empty() || bytes.len() > MAX_ENCODED_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid encoder output size",
            ));
        }
        Ok(bytes)
    }
}

impl Drop for EncoderWorkspace {
    fn drop(&mut self) {
        let source = std::mem::take(&mut self.source);
        let output = std::mem::take(&mut self.output);
        let directory = std::mem::take(&mut self.directory);
        let lease = self._lease.clone();
        dispatch_workspace_cleanup(move || {
            // Keep admission until cleanup finishes, so queued filesystem work stays bounded.
            let _lease = lease;
            // Never sweep legacy runtime files or unrecognized entries.
            let _ = fs::remove_file(source);
            let _ = fs::remove_file(output);
            let _ = fs::remove_dir(directory);
        });
    }
}

fn dispatch_workspace_cleanup(cleanup: impl FnOnce() + Send + 'static) {
    if let Ok(runtime) = tokio::runtime::Handle::try_current() {
        runtime.spawn_blocking(cleanup);
    } else {
        cleanup();
    }
}

#[cfg(test)]
#[path = "gemini_canvas_encoder_workspace_tests.rs"]
mod tests;
