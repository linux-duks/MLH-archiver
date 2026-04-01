pub mod nntp_config;
pub mod nntp_lister;
pub mod nntp_worker;

use log::{Level, log_enabled};
use nntp::NNTPStream;

pub(super) fn connect_to_nntp(address: String) -> nntp::Result<NNTPStream> {
    let mut nntp_stream = match NNTPStream::connect(address) {
        Ok(stream) => stream,
        Err(e) => {
            return Err(e);
        }
    };

    match nntp_stream.capabilities() {
        Ok(lines) => {
            if log_enabled!(Level::Debug) {
                log::debug!(
                    "server capabilities : {}",
                    lines.join(", ").replace("\n", " ")
                );
            }
        }
        Err(e) => log::error!("Failed checking server capabilities: {}", e),
    }
    return Ok(nntp_stream);
}
