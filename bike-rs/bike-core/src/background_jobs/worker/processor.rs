use async_trait::async_trait;

#[async_trait]
pub trait TaskProcessor: Send + Sync {
    fn task_type(&self) -> &str;

    fn schedule(&self) -> Option<&str> {
        None
    }

    async fn process(
        &self,
        task_id: i32,
        payload: serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}
