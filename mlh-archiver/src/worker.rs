pub trait Worker: Send {
    fn run(self: Box<Self>, receiver: crossbeam_channel::Receiver<String>) -> crate::Result<()>;
}
