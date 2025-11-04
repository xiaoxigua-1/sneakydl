use std::io::IoSlice;

use anyhow::{Context, Result};
use async_trait::async_trait;
use bytes::Bytes;
use tokio::{
    fs::File,
    io::{AsyncSeekExt, AsyncWriteExt},
};

use super::Storage;

pub struct TokioStorage {
    path: &'static str,
}

#[async_trait]
impl Storage for TokioStorage {
    type Dest = File;

    async fn create_dest(&self) -> Result<Self::Dest> {
        File::create(self.path).await.map_err(anyhow::Error::from)
    }

    async fn write_at(&self, dest: &mut Self::Dest, offset: u64, data: Vec<Bytes>) -> Result<()> {
        dest.seek(std::io::SeekFrom::Start(offset)).await?;
        let iovecs: Vec<IoSlice> = data.iter().map(|b| IoSlice::new(b)).collect();
        let mut start = 0;
        let mut offset = 0;

        while start < iovecs.len() {
            let n = dest
                .write_vectored(&iovecs[start..])
                .await
                .context("failed to perform vectored write")?;

            if n == 0 {
                anyhow::bail!("failed to write any bytes: write returned zero");
            }

            let mut remaining = n;
            while remaining > 0 && start < iovecs.len() {
                let available = iovecs[start].len() - offset;

                if remaining >= available {
                    remaining -= available;
                    start += 1;
                    offset = 0;
                } else {
                    offset += remaining;
                    remaining = 0;
                }
            }
        }

        Ok(())
    }
}
