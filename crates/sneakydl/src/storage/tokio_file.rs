use anyhow::{Context, Result};
use async_trait::async_trait;
use bytes::Bytes;
use log::trace;
use tokio::{
    fs::File,
    io::{AsyncSeekExt, AsyncWriteExt},
};

use super::Storage;

pub struct TokioStorage {
    path: &'static str,
}

impl TokioStorage {
    pub fn new(path: &'static str) -> Self {
        Self { path }
    }
}

#[async_trait]
impl Storage for TokioStorage {
    type Dest = File;

    async fn create_dest(&self) -> Result<Self::Dest> {
        File::create(self.path).await.map_err(anyhow::Error::from)
    }

    async fn write_at(&self, dest: &mut Self::Dest, offset: u64, data: Vec<Bytes>) -> Result<()> {
        dest.seek(std::io::SeekFrom::Start(offset)).await?;

        for chunk in data {
            trace!("Writing {} bytes at offset {}", chunk.len(), offset);
            dest.write_all(&chunk)
                .await
                .context("failed to write chunk")?;
        }
        trace!("Finished writing at offset {}", offset);

        Ok(())
    }
}
