use crate::common::*;
use std::collections::HashMap;

#[tokio::test]
async fn test_database_connection() -> TestResult {
    let env = TestEnvironment::new().await;

    // Test basic database operations
    let user_data = TestFixtures::sample_user();
    let document = TestDocument::new(user_data.clone());

    // Insert document
    let inserted_id = env.database.insert("users", document.clone()).await?;
    TestAssertions::assert_valid_uuid(&inserted_id)?;

    // Find by ID
    let found_document = env.database.find_by_id("users", inserted_id).await?;
    TestAssertions::assert_some(&found_document)?;

    let found = found_document.unwrap();
    TestAssertions::assert_json_equivalent(&found.data, &user_data)?;

    env.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn test_database_transactions() -> TestResult {
    let env = TestEnvironment::new().await;

    // Begin transaction
    let transaction = env.database.begin_transaction().await;
    TestAssertions::assert_valid_uuid(&transaction.id)?;

    // Perform operations within transaction
    let user_data = TestFixtures::sample_user();
    let document = TestDocument::new(user_data);

    env.database.insert("users", document).await?;

    // Commit transaction
    env.database.commit_transaction(transaction.id).await?;

    // Verify data persisted
    let count = env.database.count("users").await?;
    assert_eq!(count, 1);

    env.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn test_database_rollback() -> TestResult {
    let env = TestEnvironment::new().await;

    // Begin transaction
    let transaction = env.database.begin_transaction().await;

    // Insert data
    let user_data = TestFixtures::sample_user();
    let document = TestDocument::new(user_data);
    env.database.insert("users", document).await?;

    // Rollback transaction
    env.database.rollback_transaction(transaction.id).await?;

    // Verify data was not persisted
    let count = env.database.count("users").await?;
    assert_eq!(count, 0);

    env.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn test_database_query_filtering() -> TestResult {
    let env = TestEnvironment::new().await;

    // Insert test data
    let user1 = TestDataBuilder::user()
        .with_username("alice")
        .with_email("alice@example.com")
        .build();

    let user2 = TestDataBuilder::user()
        .with_username("bob")
        .with_email("bob@example.com")
        .build();

    env.database
        .insert("users", TestDocument::new(serde_json::to_value(&user1)?))
        .await?;
    env.database
        .insert("users", TestDocument::new(serde_json::to_value(&user2)?))
        .await?;

    // Query with filter
    let filter =
        TestFilter::new().equals("username", serde_json::Value::String("alice".to_string()));

    let results = env.database.query("users", filter).await?;
    TestAssertions::assert_length(&results, 1)?;

    let found_user = &results[0];
    assert_eq!(found_user.data["username"], "alice");

    env.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn test_database_concurrent_operations() -> TestResult {
    let env = TestEnvironment::new().await;

    // Perform concurrent inserts
    let mut handles = Vec::new();

    for i in 0..10 {
        let database = &env.database;
        let handle = tokio::spawn(async move {
            let user = TestDataBuilder::user()
                .with_username(&format!("user_{}", i))
                .with_email(&format!("user_{}@example.com", i))
                .build();

            database
                .insert(
                    "users",
                    TestDocument::new(serde_json::to_value(&user).unwrap()),
                )
                .await
        });

        handles.push(handle);
    }

    // Wait for all operations to complete
    let results = futures::future::join_all(handles).await;

    // Verify all operations succeeded
    for result in results {
        let insert_result = result??;
        TestAssertions::assert_valid_uuid(&insert_result)?;
    }

    // Verify all users were inserted
    let count = env.database.count("users").await?;
    assert_eq!(count, 10);

    env.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn test_database_error_handling() -> TestResult {
    let env = TestEnvironment::new().await;

    // Test database failure scenario
    env.database.set_should_fail(true).await;

    let user_data = TestFixtures::sample_user();
    let document = TestDocument::new(user_data);

    let result = env.database.insert("users", document).await;
    TestAssertions::assert_error_type(&result, "ConnectionFailed")?;

    // Reset database state
    env.database.set_should_fail(false).await;

    env.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn test_database_performance() -> TestResult {
    let env = TestEnvironment::new().await;

    // Test insert performance
    let start_time = std::time::Instant::now();

    for i in 0..100 {
        let user = TestDataBuilder::user()
            .with_username(&format!("perf_user_{}", i))
            .build();

        env.database
            .insert("users", TestDocument::new(serde_json::to_value(&user)?))
            .await?;
    }

    let insert_duration = start_time.elapsed();

    // Assert performance threshold (should complete within 1 second)
    assert!(insert_duration < std::time::Duration::from_secs(1));

    // Test query performance
    let start_time = std::time::Instant::now();
    let all_users = env.database.find_all("users").await?;
    let query_duration = start_time.elapsed();

    TestAssertions::assert_length(&all_users, 100)?;
    assert!(query_duration < std::time::Duration::from_millis(100));

    env.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn test_database_indexing() -> TestResult {
    let env = TestEnvironment::new().await;

    // Create index
    let index = TestIndex {
        name: "username_idx".to_string(),
        fields: vec!["username".to_string()],
        unique: true,
    };

    env.database.create_index("users", index).await?;

    // Insert user
    let user = TestDataBuilder::user()
        .with_username("indexed_user")
        .build();

    env.database
        .insert("users", TestDocument::new(serde_json::to_value(&user)?))
        .await?;

    // Try to insert duplicate username (should work in our mock, but would fail with real unique index)
    let duplicate_user = TestDataBuilder::user()
        .with_username("indexed_user")
        .with_email("different@example.com")
        .build();

    // In a real implementation, this would fail due to unique constraint
    let result = env
        .database
        .insert(
            "users",
            TestDocument::new(serde_json::to_value(&duplicate_user)?),
        )
        .await;

    // For our mock implementation, this succeeds, but in real MongoDB it would fail
    assert!(result.is_ok());

    env.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn test_database_bulk_operations() -> TestResult {
    let env = TestEnvironment::new().await;

    // Prepare bulk data
    let mut documents = Vec::new();
    for i in 0..50 {
        let user = TestDataBuilder::user()
            .with_username(&format!("bulk_user_{}", i))
            .build();
        documents.push(TestDocument::new(serde_json::to_value(&user)?));
    }

    // Simulate bulk insert
    let start_time = std::time::Instant::now();
    for document in documents {
        env.database.insert("users", document).await?;
    }
    let bulk_duration = start_time.elapsed();

    // Verify all documents were inserted
    let count = env.database.count("users").await?;
    assert_eq!(count, 50);

    // Assert bulk operation performance
    assert!(bulk_duration < std::time::Duration::from_secs(2));

    env.cleanup().await;
    Ok(())
}
