use crate::error::DownloadError;
use nizziel::{ download, Downloads };
use crate::state::State;
use crate::paths::Paths;
use crate::Instance;

pub trait DownloadHelper {
    fn state(&self) -> &State;
    fn paths(&self) -> &Paths;
}

impl <T> DownloadHelper for Instance<T> {
    fn state(&self) -> &State {
        &self.state
    }

    fn paths(&self) -> &Paths {
        &self.paths
    }
}

pub trait DownloadSequence: DownloadHelper {
    fn collect_urls(&mut self) -> Result<Downloads, DownloadError>;
    fn create_state(&mut self) -> Result<(), DownloadError>;

    fn download(&mut self) -> Result<(), DownloadError> {
        self.create_state()?;
        self.state().write(self.paths().get("instance")?)?;

        let urls = self.collect_urls()?;
        self.spawn_thread(urls)
    }

    fn spawn_thread(&mut self, dls: Downloads) -> Result<(), DownloadError> {
        log::info!("Downloading!");

        let before = std::time::Instant::now();
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(10)
            .enable_io()
            .enable_time()
            .build()?;

        rt.block_on(
            async move {
                download(dls).await.unwrap();
            }
        );

        log::info!("Time taken: {:.2?}", before.elapsed());
        Ok(())
    }
}