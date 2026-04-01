use std::sync::Arc;

pub trait Worker: Send + Sync {
    fn run(self: Arc<Self>, receiver: crossbeam_channel::Receiver<String>) -> crate::Result<()>;
}
