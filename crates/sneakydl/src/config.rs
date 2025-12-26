#[derive(Debug)]
pub struct Config {
    /// N Set maximum number of parallel downloads for
    /// every static HTTP URL
    pub max_concurrent: u32,
    /// NUM The maximum number of connections to one
    /// server for each download.
    pub max_connection: u8,
    pub split_strategy: SplitStrategy,
}

#[derive(Debug, Clone)]
pub enum SplitStrategy {
    BySize(usize),
    ByCount(usize),
    Single,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_concurrent: 5,
            max_connection: 1,
            split_strategy: SplitStrategy::BySize(5 * 1024 * 1024),
        }
    }
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn max_concurrent(mut self, max: u32) -> Self {
        self.max_concurrent = max;
        self
    }

    pub fn max_connection(mut self, max: u8) -> Self {
        self.max_connection = max;
        self
    }

    pub fn split_strategy(mut self, strategy: SplitStrategy) -> Self {
        self.split_strategy = strategy;
        self
    }
}
