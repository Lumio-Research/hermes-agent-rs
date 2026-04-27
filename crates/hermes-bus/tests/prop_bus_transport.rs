use hermes_bus::messages::{AgentRequest, StatusQuery};
use hermes_bus::transport::BusTransport;
use hermes_bus::{BusMessage, InProcessTransport};
use hermes_core::traits::AgentOverrides;
use proptest::prelude::*;

fn arb_bus_message() -> impl Strategy<Value = BusMessage> {
    prop_oneof![
        (".*", ".*", any::<bool>()).prop_map(|(sid, text, stream)| {
            BusMessage::AgentRequest(AgentRequest {
                request_id: format!("req-{sid}"),
                session_id: sid,
                text,
                overrides: AgentOverrides::default(),
                stream,
            })
        }),
        any::<bool>().prop_map(|v| {
            BusMessage::StatusQuery(StatusQuery {
                include_platforms: v,
                include_sessions: !v,
            })
        }),
    ]
}

proptest! {
    #[test]
    fn inprocess_transport_round_trip(msg in arb_bus_message()) {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let recv = rt.block_on(async {
            let (a, b) = InProcessTransport::new(8);
            a.send(msg.clone()).await.expect("send");
            b.receive().await.expect("receive")
        });
        prop_assert_eq!(recv, msg);
    }
}
