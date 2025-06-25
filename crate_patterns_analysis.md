# How Popular Crates Handle Similar Patterns

## 1. Serde's Approach

Serde doesn't generate the types it uses (like `Serialize`, `Deserialize`, `Serializer`, etc.). Instead:

- All types are manually defined in the main crate
- Derive macros reference these existing types
- Compile-time validation happens naturally through type checking

```rust
// In serde crate
pub trait Serialize { ... }
pub trait Deserialize<'de> { ... }

// In serde_derive
#[derive(Serialize, Deserialize)]
struct MyStruct { ... }
// Generated code references serde::Serialize, serde::Deserialize
```

## 2. Diesel's Approach

Diesel uses a similar pattern for its query builder:

- Core types like `table!` macro generate structs
- Derive macros like `Queryable` reference existing types
- Schema is defined separately and referenced by derives

```rust
// Schema definition
table! {
    users (id) {
        id -> Integer,
        name -> Text,
    }
}

// Derive references the schema
#[derive(Queryable)]
struct User {
    id: i32,
    name: String,
}
```

## 3. Strum's Approach

Strum generates associated types but with a different pattern:

- `EnumDiscriminants` generates a new enum alongside the original
- The generated enum has a predictable name pattern
- Both enums exist in the same scope

```rust
#[derive(EnumDiscriminants)]
#[strum_discriminants(derive(EnumString))]
enum MyEnum {
    Variant1,
    Variant2(String),
}
// Generates: MyEnumDiscriminants enum
```

## 4. Clap's Approach

Clap v3+ uses derives that reference types from the main crate:

```rust
use clap::Parser;

#[derive(Parser)]
struct Args {
    #[clap(short, long)]
    name: String,
}
// References clap::Parser trait, doesn't generate new types
```

## 5. SQLx's Approach

SQLx validates SQL at compile time against actual database schemas:

- Uses compile-time feature flags
- Queries are validated during macro expansion
- Generates type-safe code based on database schema

```rust
sqlx::query!("SELECT id, name FROM users WHERE id = ?", user_id)
// Validates against actual database at compile time
```

## Key Patterns Observed

### 1. **Separation of Definition and Usage**
Most successful crates separate type definitions from their usage in macros:
- Types are defined in the main crate
- Macros reference these types
- No attempt to generate the referenced types

### 2. **Predictable Naming**
When types ARE generated (like Strum), they follow predictable patterns:
- `EnumDiscriminants` → `{EnumName}Discriminants`
- Clear documentation about what gets generated

### 3. **Compile-Time Validation**
All crates ensure type safety at compile time:
- Invalid references cause compilation errors
- Clear error messages guide users

### 4. **Documentation as Contract**
The types serve as documentation:
- Users can look up available variants
- IDE autocomplete works well
- No hidden or generated types to discover

## Recommendation for grafton-visca

Following these established patterns, the best approach is:

1. **Keep ResponseType manually defined** - Like serde's traits
2. **Validate at compile time** - Natural type checking
3. **Clear documentation** - ResponseType enum documents all variants
4. **No magic** - Users can see exactly what's available

This approach has proven successful across the Rust ecosystem and provides the best developer experience.