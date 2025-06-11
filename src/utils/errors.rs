use anyhow::anyhow;

pub trait AnyHowErr<T> {
    fn anyhow_err(self, msg: &str) -> anyhow::Result<T>;
}

impl<T, E> AnyHowErr<T> for Result<T, E>
where
    E: std::fmt::Display + Send + Sync + 'static,
{
    fn anyhow_err(self, msg: &str) -> anyhow::Result<T> {
        self.map_err(|err| {
            error!("{}: {}", msg, err);
            anyhow!("{}: {}", msg, err)
        })
    }
}

