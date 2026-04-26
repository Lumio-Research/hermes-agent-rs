use hermes_agent::session_persistence::SessionPersistence;
use hermes_core::{Message, MessageRole};
use proptest::prelude::*;
use tempfile::tempdir;

fn arb_message() -> impl Strategy<Value = Message> {
    prop_oneof![
        ".*".prop_map(Message::user),
        ".*".prop_map(Message::assistant),
        ".*".prop_map(Message::system),
    ]
}

proptest! {
    // Feature: unified-runtime-architecture, Property 1: Session persistence round-trip
    #[test]
    fn session_persistence_round_trip(messages in prop::collection::vec(arb_message(), 0..20)) {
        let tmp = tempdir().expect("tempdir");
        let home = tmp.keep();
        let sp = SessionPersistence::new(&home);
        let _ = sp.ensure_db();

        sp.persist_session("prop-sid", &messages, Some("openai:gpt-4o"), Some("test"), None, None)
            .expect("persist");
        let loaded = sp.load_session("prop-sid").expect("load");

        prop_assert_eq!(loaded.len(), messages.len());
        for (a, b) in loaded.iter().zip(messages.iter()) {
            prop_assert_eq!(a.role, b.role);
            prop_assert_eq!(&a.content, &b.content);
            prop_assert_eq!(&a.tool_call_id, &b.tool_call_id);
            prop_assert_eq!(&a.tool_calls, &b.tool_calls);
        }
    }

    // Feature: unified-runtime-architecture, Property 2: Session count matches persisted sessions
    #[test]
    fn session_count_matches_distinct_ids(ids in prop::collection::hash_set("[a-z0-9]{1,16}", 0..40)) {
        let tmp = tempdir().expect("tempdir");
        let home = tmp.keep();
        let sp = SessionPersistence::new(&home);
        let _ = sp.ensure_db();

        for sid in &ids {
            let msg = vec![Message {
                role: MessageRole::User,
                content: Some(format!("hello-{sid}")),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: None,
                cache_control: None,
            }];
            sp.persist_session(sid, &msg, Some("openai:gpt-4o"), Some("test"), None, None)
                .expect("persist");
        }

        let conn = rusqlite::Connection::open(home.join("sessions.db")).expect("open sqlite");
        let count: usize = conn
            .query_row("SELECT COUNT(DISTINCT id) FROM sessions", [], |row| row.get(0))
            .expect("count");
        prop_assert_eq!(count, ids.len());
    }
}
