# 😵‍💫 oxymorph

Attribute macro generating view, create and patch payload types from a single canonical struct.

## The problem

Let's say I want to write a `User` serializable struct for my API. I want to have an internal model that has all the fields to send to the user when they fetch their own data:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    id: i32,
    username: String,
    name: String,
    email: String,
    bio: Option<String>,
    age: Option<u8>,
    created_at: u64,
}
```

But if someone wants to fetch another user, I don't want to send them the `email` field which should only be visible to each user themselves. So I have to make a separate struct for the view:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserView {
    id: i32,
    username: String,
    name: String,
    bio: Option<String>,
    age: Option<u8>,
    created_at: u64,
}
```

Great! Now we need to write the endpoint to create a user. But the user shouldn't have to provide `created_at` or `id`. And maybe I want to allow them to set the `email` on create, but not update it later. Or even make the `age` field optional AND nullable, meaning if they don't include it, we fall back to a value, but if they include it with `null` then it gets set to `null`. So I end up with:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserCreate {
    username: String,
    name: String,
    email: String,
    bio: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    age: Option<Option<u8>>,
}
```

(And yes, you have to make that `double_option` helper yourself, because serde gives you no way to tell `{}` from `{"age": null}` otherwise.)

Now we want to allow updates, specifically patches. But what if I want to allow users to update their `name`, `bio` and `age`, but not their `id`, `username`, `email` or `created_at`? Then I have to make a separate struct for the patch request and it has to have all fields optional:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserPatch {
    name: Option<String>,
    bio: Option<String>,
    age: Option<u8>,
}
```

That's a lot of boilerplate! And then you have to write the code to convert between all these structs and the code to apply the patch to the model, etc. The latter is especially annoying, because your PATCH handler has to walk each field by hand checking `if let Some(name) = patch.name { user.name = name; }`.

To make things worse, the day someone adds a new field to one of the 4 DTOs, you don't get a compile-time error if you forget to add it to the others or to handle it in the patch application code.

And on top of all that, when a client sends `{"age": null}` you have to distinguish "clear the age" from "I don't want to patch this field", which means `Option<Option<T>>`, `#[serde(default, deserialize_with = "double_option")]` and three nested branches for each field!

`oxymorph` collapses all of that into one annotation:

```rust
use oxymorph::{Patch, model};
use serde::{Deserialize, Serialize};

#[oxymorph::model]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    #[oxymorph(server_only)]
    id: i32,
    #[oxymorph(immutable)]
    username: String,
    name: String,
    #[oxymorph(hide(view), immutable)]
    email: String,
    bio: Option<String>,
    age: Option<u8>,
    #[oxymorph(server_only)]
    created_at: u64,
}
```

You get `UserDelta` (PATCH payload), `UserDraft` (create payload) and `UserView` (read projection) generated for free with absent-vs-null distinguished at the type level via `Patch<T>`.
