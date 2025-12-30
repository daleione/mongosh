use super::completion::CommandCompleter;
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

#[test]
fn test_cursor_state_creation() {
    let cursor = CursorState::new(
        "test_collection".to_string(),
        mongodb::bson::doc! {},
        crate::parser::FindOptions::default(),
        Some(100),
    );

    assert_eq!(cursor.collection, "test_collection");
    assert_eq!(cursor.documents_retrieved, 0);
    assert_eq!(cursor.total_matched, Some(100));
    assert!(cursor.has_more);
}

#[test]
fn test_cursor_state_update() {
    let mut cursor = CursorState::new(
        "test_collection".to_string(),
        mongodb::bson::doc! {},
        crate::parser::FindOptions::default(),
        Some(100),
    );

    cursor.update(20, Some(100));
    assert_eq!(cursor.documents_retrieved, 20);
    assert_eq!(cursor.get_skip(), 20);
}

#[test]
fn test_shared_state_cursor_operations() {
    let state = SharedState::new("test".to_string());

    assert!(!state.has_active_cursor());
    assert!(state.get_cursor_state().is_none());

    let cursor = CursorState::new(
        "test_collection".to_string(),
        mongodb::bson::doc! {},
        crate::parser::FindOptions::default(),
        None,
    );

    state.set_cursor_state(Some(cursor));
    assert!(state.has_active_cursor());
    assert!(state.get_cursor_state().is_some());

    state.clear_cursor_state();
    assert!(!state.has_active_cursor());
}

#[test]
fn test_command_completer() {
    let completer = CommandCompleter::new();
    let completions = completer.get_completions("show");
    assert!(!completions.is_empty());
    assert!(completions.iter().all(|c| c.starts_with("show")));
}

#[test]
fn test_command_completer_custom() {
    let completer = CommandCompleter::with_commands(["alpha", "beta", "gamma"]);
    let completions = completer.get_completions("b");
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0], "beta");
}
