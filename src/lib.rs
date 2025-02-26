pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub mod hash;
pub mod fs;
pub mod objects;
pub mod dirtree;
pub mod root;
pub mod utils;
