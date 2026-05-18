use oxymorph::Patch;
use serde::{Deserialize, Serialize};

#[oxymorph::model]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    #[oxymorph(immutable)]
    id: String,

    name: String,

    #[oxymorph(hide(view))]
    email: String,

    bio: Option<String>,

    #[oxymorph(server_only)]
    created_at: u64,
}

#[test]
fn test_delta_serialization() {
    let delta = UserDelta {
        name: Patch::Absent,
        email: Patch::Set(String::new()),
        bio: Patch::Set(None),
    };
    let json = serde_json::to_string(&delta).unwrap();
    assert_eq!(json, r#"{"email":"","bio":null}"#);
}
