pub mod config;
pub mod downloader;
pub mod result;

use std::sync::Arc;

use crate::config::Config;

#[derive(Debug, Default)]
pub struct Sneakydl {
    config: Arc<Config>,
}

pub struct SneakydlBuilder {
    config: Config,
}

impl Sneakydl {
    pub fn new() -> Self {
        Sneakydl::default()
    }

    pub fn builder() -> SneakydlBuilder {
        SneakydlBuilder::new(Config::default())
    }
}

impl SneakydlBuilder {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn split(mut self, split: u32) -> Self {
        self.config.split = split;

        self
    }

    pub fn min_split_size(mut self, min_split_size: u32) -> Self {
        self.config.min_split_size = min_split_size;

        self
    }

    pub fn max_concurrent(mut self, max_concurrent: u32) -> Self {
        self.config.max_concurrent = max_concurrent;

        self
    }

    pub fn max_connection(mut self, max_connnection: u8) -> Self {
        self.config.max_connection = max_connnection;

        self
    }

    pub fn build(self) -> Sneakydl {
        Sneakydl {
            config: Arc::new(self.config),
        }
    }
}
