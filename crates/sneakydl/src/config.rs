#[derive(Debug)]
pub struct Config {
    /// Download a file using N connections. If more
    /// than N URIs are given, first N URIs are used and
    /// remaining URLs are used for backup. If less than
    /// N URIs are given, those URLs are used more than
    /// once so that N connections total are made
    /// simultaneously
    pub split: u32,
    /// sneakydl does not split less than 2*SIZE byte range.
    pub min_split_size: u32,

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
            split: 5,
            // 20MB
            min_split_size: 20 * 1024 * 1024,
            max_concurrent: 5,
            max_connection: 1,
            split_strategy: SplitStrategy::BySize(5 * 1024 * 1024),
        }
    }
}
