use async_trait::async_trait;
use flocker::{
    docker::{ContainerConfig, DockerOperations},
    state::{ContainerInfo, State},
    ui::{ContainerUI, UserInterface},
    ContainerStatus, Result,
};
use serial_test::serial;
use tempfile::tempdir;

// Mock UserInterface implementation for testing
struct MockUserInterface;

impl UserInterface for MockUserInterface {
    fn get_string_input(&self, _prompt: &str) -> Result<String> {
        Ok("test".to_string())
    }

    fn get_string_input_with_default(&self, _prompt: &str, default: &str) -> Result<String> {
        Ok(default.to_string())
    }

    fn get_bool_input(&self, _prompt: &str, default: bool) -> Result<bool> {
        Ok(default)
    }

    fn get_selection<T: ToString>(&self, _prompt: &str, _items: &[T]) -> Result<usize> {
        // Return 1 to select the first container (index 0 is "Create new container")
        Ok(1)
    }

    fn display_success(&self, _message: &str) {}
    fn display_warning(&self, _message: &str) {}
}

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
    async fn get_container_status(&self, _container_id: &str) -> flocker::Result<ContainerStatus> {
        Ok(self.container_status.clone())
    }

    async fn start_container(&self, _container_id: &str) -> flocker::Result<()> {
        Ok(())
    }

    async fn stop_container(&self, _container_id: &str) -> flocker::Result<()> {
        Ok(())
    }

    async fn remove_container(&self, _container_id: &str) -> flocker::Result<()> {
        Ok(())
    }

    async fn create_and_start_container(
        &self,
        _image_tag: &flocker::cli::Tag,
        _config: &ContainerConfig,
        _name: &str,
    ) -> flocker::Result<String> {
        Ok("test-container-id".to_string())
    }

    async fn list_ledgers(
        &self,
        _container_id: &str,
    ) -> flocker::Result<Vec<flocker::docker::LedgerInfo>> {
        Ok(Vec::new())
    }

    async fn get_ledger_details(
        &self,
        _container_id: &str,
        _path: &str,
    ) -> flocker::Result<String> {
        Ok("{}".to_string())
    }

    async fn delete_ledger(&self, _container_id: &str, _path: &str) -> flocker::Result<()> {
        Ok(())
    }

    async fn pull_image(&self, _tag: &str) -> flocker::Result<()> {
        Ok(())
    }

    async fn get_image_by_tag(
        &self,
        _tag_str: &str,
    ) -> flocker::Result<flocker::docker::FlureeImage> {
        unimplemented!("Not needed for these tests")
    }

    async fn list_local_images(&self) -> flocker::Result<Vec<flocker::docker::FlureeImage>> {
        unimplemented!("Not needed for these tests")
    }

    async fn get_container_stats(&self, _container_id: &str) -> flocker::Result<String> {
        Ok("CONTAINER ID        CPU %               MEM USAGE / LIMIT     MEM %\ntest-container      0.00%               10.0MB / 100.0MB      10.00%".to_string())
    }

    async fn get_container_logs(
        &self,
        _container_id: &str,
        _tail: Option<&str>,
    ) -> flocker::Result<String> {
        Ok("Mock container logs for testing".to_string())
    }
}

fn create_test_container(id: &str, name: &str, port: u16) -> ContainerInfo {
    ContainerInfo::new(
        id.to_string(),
        name.to_string(),
        port,
        None,
        true,
        "latest".to_string(),
    )
}

#[tokio::test]
async fn test_container_workflow() {
    // Set up temporary directory for state
    let temp_dir = tempdir().unwrap();
    std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

    // Create test state with a container
    let mut state = State::default();
    let container = create_test_container("test1", "test-container", 8090);
    state.add_container(container).unwrap();

    // Create UI with test state and mock UI
    let ui = ContainerUI::with_ui(state, MockUserInterface);

    // Test running container workflow
    let running_status = ContainerStatus::Running {
        id: "test1".to_string(),
        name: "test-container".to_string(),
        port: 8090,
        data_dir: None,
        started_at: Some("2024-01-01T00:00:00Z".to_string()),
    };
    let docker = MockDockerManager::new(running_status);

    // Select container should return Some with the container ID
    let result = ui.select_container(&docker).await.unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "test1");

    // Test stopped container workflow
    let stopped_status = ContainerStatus::Stopped {
        id: "test1".to_string(),
        name: "test-container".to_string(),
        last_start: Some("2024-01-01T00:00:00Z".to_string()),
    };
    let docker = MockDockerManager::new(stopped_status);

    // Select container should still return Some with the container ID
    let result = ui.select_container(&docker).await.unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "test1");

    // Test not found container workflow
    let docker = MockDockerManager::new(ContainerStatus::NotFound);

    // Select container should return None when container is not found
    let result = ui.select_container(&docker).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_empty_state_workflow() {
    // Set up temporary directory for state
    let temp_dir = tempdir().unwrap();
    std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

    // Create UI with empty state and mock UI
    let ui = ContainerUI::with_ui(State::default(), MockUserInterface);
    let docker = MockDockerManager::new(ContainerStatus::NotFound);

    // Select container should return None when no containers exist
    let result = ui.select_container(&docker).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
#[serial]
async fn test_container_action_workflow() {
    // Set up temporary directory for state
    let temp_dir = tempdir().unwrap();
    std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

    // Create test state with a container
    let mut state = State::default();
    let container = create_test_container("test1", "test-container", 8090);
    state.add_container(container).unwrap();

    // Create UI with test state and mock UI
    let ui = ContainerUI::with_ui(state, MockUserInterface);

    // Test running container actions
    let action = ui.display_action_menu(true).unwrap();
    assert!(matches!(action, 0..=5));

    // Test stopped container actions
    let action = ui.display_action_menu(false).unwrap();
    assert!(matches!(action, 0..=2));
}
