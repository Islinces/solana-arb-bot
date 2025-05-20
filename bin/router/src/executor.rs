use async_trait::async_trait;
use burberry::Executor;

#[derive(Clone, Debug)]
pub enum ExecutorType {
    Simple(()),
}

pub struct SimpleExecutor;

#[async_trait]
impl Executor<()> for SimpleExecutor {
    async fn execute(&self, action: ()) -> eyre::Result<()> {
        todo!()
    }
}
