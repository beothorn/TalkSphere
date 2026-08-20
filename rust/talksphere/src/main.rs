fn main() {
    tracing_subscriber::fmt::init();
    tracing::trace!(">main(): starting TalkSphere");
    tracing::info!("TalkSphere is running");
    tracing::trace!("<main(): TalkSphere finished");
}
