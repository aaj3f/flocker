use async_trait::async_trait;
use flocker::{
    cli::{hub::Tag, CliState},
    docker::{ContainerConfig, DockerOperations, FlureeImage},
    state::ContainerInfo,
    ContainerStatus, Result,
};
use tempfile::tempdir;

// Mock DockerManager for testing
#[derive(Clone)]
struct MockDockerManager {
    container_status: ContainerStatus,
}

impl MockDockerManager {
    fn new(status: ContainerStatus) -> Self {
        Self {
            container_status: status,
        }
    }
}

#[async_trait]
impl DockerOperations for MockDockerManager {
    async fn get_container_status(&self, _container_id: &str) -> Result<ContainerStatus> {
        Ok(self.container_status.clone())
    }

    async fn start_container(&self, _container_id: &str) -> Result<()> {
        Ok(())
    }

    async fn stop_container(&self, _container_id: &str) -> Result<()> {
        Ok(())
    }

    async fn remove_container(&self, _container_id: &str) -> Result<()> {
        Ok(())
    }

    async fn create_and_start_container(
        &self,
        image_tag: &Tag,
        _config: &ContainerConfig,
        name: &str,
    ) -> Result<ContainerInfo> {
        Ok(ContainerInfo::new(
            "test-container-id".to_string(),
            name.to_string(),
            8090,
            None,
            None,
            image_tag.name().to_string(),
        ))
    }

    async fn list_ledgers(&self, _container_id: &str) -> Result<Vec<flocker::docker::LedgerInfo>> {
        Ok(Vec::new())
    }

    async fn get_ledger_details(&self, _container_id: &str, _path: &str) -> Result<String> {
        Ok("{}".to_string())
    }

    async fn delete_ledger(&self, _container_id: &str, _path: &str) -> Result<()> {
        Ok(())
    }

    async fn pull_image(&self, _tag: &str) -> Result<()> {
        Ok(())
    }

    async fn get_image_by_tag(&self, _tag_str: &str) -> Result<FlureeImage> {
        unimplemented!("Not needed for these tests")
    }

    async fn list_local_images(&self) -> Result<Vec<FlureeImage>> {
        unimplemented!("Not needed for these tests")
    }

    async fn get_container_stats(&self, _container_id: &str) -> Result<String> {
        Ok("CONTAINER ID        CPU %               MEM USAGE / LIMIT     MEM %\ntest-container      0.00%               10.0MB / 100.0MB      10.00%".to_string())
    }

    async fn get_container_logs(&self, _container_id: &str, _tail: Option<&str>) -> Result<String> {
        Ok("Mock container logs for testing".to_string())
    }
}

fn create_test_container(id: &str, name: &str, port: u16) -> ContainerInfo {
    ContainerInfo::new(
        id.to_string(),
        name.to_string(),
        port,
        None,
        None,
        "latest".to_string(),
    )
}

#[tokio::test]
async fn test_container_workflow() {
    // Set up temporary directory for state
    let temp_dir = tempdir().unwrap();
    std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

    // Create test container
    let container = create_test_container("test1", "test-container", 8090);

    // Create CLI state with test container
    let mut cli = CliState::new();
    cli.add_container(container.clone()).unwrap();

    // Test running container workflow
    let running_status = ContainerStatus::Running {
        id: "test1".to_string(),
        name: "test-container".to_string(),
        port: 8090,
        data_dir: None,
        config_dir: None,
        started_at: Some("2024-01-01T00:00:00Z".to_string()),
    };
    let docker = MockDockerManager::new(running_status);

    // Try running existing container should return Some with the container ID
    let result = cli.try_running_existing_container(&docker).await.unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "test1");

    // Test stopped container workflow
    let stopped_status = ContainerStatus::Stopped {
        id: "test1".to_string(),
        name: "test-container".to_string(),
        last_start: Some("2024-01-01T00:00:00Z".to_string()),
    };
    let docker = MockDockerManager::new(stopped_status);

    // Try running existing container should still return Some with the container ID
    let result = cli.try_running_existing_container(&docker).await.unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "test1");

    // Test not found container workflow
    let docker = MockDockerManager::new(ContainerStatus::NotFound);

    // Try running existing container should return None when container is not found
    let result = cli.try_running_existing_container(&docker).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_empty_state_workflow() {
    // Set up temporary directory for state
    let temp_dir = tempdir().unwrap();
    std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

    // Create CLI state with empty state
    let mut cli = CliState::new();
    let docker = MockDockerManager::new(ContainerStatus::NotFound);

    // Try running existing container should return None when no containers exist
    let result = cli.try_running_existing_container(&docker).await.unwrap();
    assert!(result.is_none());
}
