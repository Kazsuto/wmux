---
title: Trait-Based Dependency Injection for Testable Code
impact: MEDIUM
impactDescription: enables isolated testing without network, filesystem, or external services
tags: testing, dependency-injection, traits, mockall, mocking
---

## Trait-Based Dependency Injection for Testable Code

Design code with trait-based boundaries at I/O points. Use mockall for auto-generated mocks or real in-memory implementations.

**Incorrect (hardcoded I/O dependency — requires network, slow, flaky):**

```rust
// Directly calls reqwest::get — cannot be tested without a live network
pub async fn get_user_name(user_id: u64) -> Result<String, AppError> {
    let url = format!("https://api.example.com/users/{user_id}");
    let user: User = reqwest::get(&url)
        .await?
        .json()
        .await?;
    Ok(user.name)
}
```

**Correct (trait boundary at I/O — swappable in tests):**

```rust
#[cfg_attr(test, automock)]
#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn fetch_user(&self, user_id: u64) -> Result<User, AppError>;
}

pub async fn get_user_name(repo: &dyn UserRepository, user_id: u64) -> Result<String, AppError> {
    let user = repo.fetch_user(user_id).await?;
    Ok(user.name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;

    #[tokio::test]
    async fn returns_name_for_known_user() {
        let mut mock = MockUserRepository::new();
        mock.expect_fetch_user()
            .with(eq(42u64))
            .returning(|_| Ok(User { name: "Alice".to_string() }));

        let name = get_user_name(&mock, 42).await.unwrap();
        assert_eq!(name, "Alice");
    }
}
```

**Note:** Prefer real implementations (in-memory SQLite) over mocks when possible. Use .times(n) sparingly. Since Rust 1.75, native `async fn` in traits works for static dispatch without `#[async_trait]`. The `#[async_trait]` crate is only needed when using `dyn Trait` (which the example above does). For generic/static dispatch, use native async traits directly — mockall supports them without `#[async_trait]`:

```rust
#[cfg_attr(test, automock)]
pub trait UserRepository: Send + Sync {
    async fn fetch_user(&self, user_id: u64) -> Result<User, AppError>;
}

pub async fn get_user_name<R: UserRepository>(repo: &R, user_id: u64) -> Result<String, AppError> {
    let user = repo.fetch_user(user_id).await?;
    Ok(user.name)
}
```
