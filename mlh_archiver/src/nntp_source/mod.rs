//! NNTP (Network News Transfer Protocol) source implementation.
//!
//! This module provides the NNTP-specific worker implementation for fetching
//! emails from NNTP servers. It includes:
//!
//! - [`nntp_config`] - Configuration struct for NNTP connections
//! - [`nntp_lister`] - Functions to retrieve available mailing lists
//! - [`nntp_worker`] - Worker implementation that fetches emails
//! - [`nntp_utils`] - Reusable utility functions for NNTP operations
//!
//! # Architecture
//!
//! The NNTP worker:
//! 1. Connects to the NNTP server on creation
//! 2. Uses `RefCell` for interior mutability of the connection
//! 3. Checks shutdown flag during long operations
//! 4. Tracks progress via `__last_article_number` files
//! 5. Handles reconnection on network errors

pub mod nntp_config;
pub mod nntp_lister;
pub mod nntp_utils;
pub mod nntp_worker;

// Re-export commonly used types for convenience
pub use nntp_config::NntpConfig;
pub use nntp_utils::{
    connect_to_nntp_server, get_group_info, retrieve_groups_info, retrieve_lists_with_connection,
};
