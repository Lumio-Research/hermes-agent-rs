use hermes_bus::messages::*;
use hermes_core::traits::AgentOverrides;
use proptest::prelude::*;

fn arb_bus_message() -> impl Strategy<Value = BusMessage> {
    let small = ".*";
    prop_oneof![
        (
            small.prop_map(|s| s.to_string()),
            small.prop_map(|s| s.to_string()),
            prop::option::of(small.prop_map(|s| s.to_string())),
            prop::option::of(small.prop_map(|s| s.to_string())),
            any::<bool>(),
        )
            .prop_map(|(platform, chat_id, user_id, text, is_dm)| {
                BusMessage::PlatformIncoming(PlatformIncoming {
                    platform,
                    chat_id,
                    user_id: user_id.unwrap_or_else(|| "u".to_string()),
                    text: text.unwrap_or_else(|| "hello".to_string()),
                    message_id: None,
                    is_dm,
                })
            }),
        (
            small.prop_map(|s| s.to_string()),
            small.prop_map(|s| s.to_string()),
            prop::option::of(small.prop_map(|s| s.to_string())),
            prop::option::of(small.prop_map(|s| s.to_string())),
            any::<bool>(),
        )
            .prop_map(|(sid, text, model, persona, stream)| {
                BusMessage::AgentRequest(AgentRequest {
                    request_id: format!("req-{sid}"),
                    session_id: sid,
                    text,
                    overrides: AgentOverrides {
                        model,
                        personality: persona,
                    },
                    stream,
                })
            }),
        (
            small.prop_map(|s| s.to_string()),
            small.prop_map(|s| s.to_string()),
            any::<u16>(),
        )
            .prop_map(|(sid, text, n)| {
                BusMessage::AgentResponse(AgentResponse {
                    request_id: format!("req-{sid}"),
                    session_id: sid,
                    text,
                    message_count: n as usize,
                    error: None,
                    done: true,
                })
            }),
        (small.prop_map(|s| s.to_string()), any::<bool>()).prop_map(|(sid, reset)| {
            BusMessage::SessionQuery(SessionQuery {
                request_id: format!("req-{sid}"),
                session_id: sid,
                action: if reset {
                    SessionQueryAction::ResetSession
                } else {
                    SessionQueryAction::GetMessages
                },
            })
        }),
        any::<bool>().prop_map(|include| {
            BusMessage::StatusQuery(StatusQuery {
                include_platforms: include,
                include_sessions: !include,
            })
        }),
    ]
}

proptest! {
    #[test]
    fn bus_message_serde_round_trip(msg in arb_bus_message()) {
        let json = serde_json::to_string(&msg).expect("serialize");
        let parsed: BusMessage = serde_json::from_str(&json).expect("deserialize");
        prop_assert_eq!(parsed, msg);
    }
}
