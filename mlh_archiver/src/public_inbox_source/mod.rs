//! Public Inbox Source Module
//!
//! This module provides functionality for interfacing with public-inbox email archives.
//! It includes components for:
//! - Configuration (`pi_config`)
//! - Listing available inboxes (`pi_lister`)
//! - Utility functions for git operations and email ID handling (`pi_utils`)
//! - Worker implementation for processing inboxes (`pi_worker`)
//!
//! The module supports both V1 (single repository) and V2 (epoch-based) public-inbox layouts.
//! For V2 inboxes, it handles epoch-aware sequential email ID generation in the format:
//! `{10-digit-padded}-e{epoch}-{7-char-short-sha}.eml` (e.g., `0000000001-e1-d3ed66e.eml`)
//!
//! Key features:
//! - Automatic detection of public-inbox directory structure
//! - Support for resuming processing from a specific email using progress tracking
//! - Article range filtering for processing specific subsets of emails
//! - Integration with the archive writer for storing processed emails
//! - Grokmirror integration placeholder for future repository discovery

pub mod pi_config;
pub mod pi_lister;
pub mod pi_utils;
pub mod pi_worker;
