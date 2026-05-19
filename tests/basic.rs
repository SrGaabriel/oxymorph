use oxymorph::Patch;
use serde::{Deserialize, Serialize};

mod users {
    #![allow(dead_code)]
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "users")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: String,
        pub name: String,
        pub email: String,
        pub bio: Option<String>,
        pub created_at: u64,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[oxymorph::model(delta, view, draft, sea_orm(entity = users))]
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
