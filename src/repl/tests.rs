use super::*;

#[test]
fn test_shared_state_creation() {
    let state = SharedState::new("test".to_string());
    assert_eq!(state.get_database(), "test");
    assert!(!state.is_connected());
}

#[test]
fn test_shared_state_connection() {
    let mut state = SharedState::new("test".to_string());
    state.set_connected(Some("5.0.0".to_string()));
    assert!(state.is_connected());
}

#[test]
fn test_shared_state_database_change() {
    let mut state = SharedState::new("test".to_string());
    state.set_database("newdb".to_string());
    assert_eq!(state.get_database(), "newdb");
}

// Note: CursorState tests that require a real MongoDB cursor have been removed.
// Integration tests with actual MongoDB connection should test cursor functionality.

#[tokio::test]
async fn test_shared_state_cursor_operations() {
    let state = SharedState::new("test".to_string());

    // Initially no cursor
    assert!(!state.has_cursor().await);

    // Clear cursor (should not panic even when no cursor exists)
    state.clear_cursor().await;
    assert!(!state.has_cursor().await);
}
