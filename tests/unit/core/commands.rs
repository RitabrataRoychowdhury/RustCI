use crate::common::*;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn test_command_creation() -> TestResult {
    let command = TestDataBuilder::command()
        .with_type("TestCommand")
        .with_payload(json!({"key": "value"}))
        .build();

    TestAssertions::assert_valid_uuid(&command.id)?;
    TestAssertions::assert_not_empty(&command.command_type)?;
    assert_eq!(command.command_type, "TestCommand");

    Ok(())
}

#[tokio::test]
async fn test_command_with_user() -> TestResult {
    let user_id = Uuid::new_v4();
    let command = TestDataBuilder::command()
        .with_type("UserCommand")
        .with_user(user_id)
        .build();

    TestAssertions::assert_some(&command.user_id)?;
    assert_eq!(command.user_id.unwrap(), user_id);

    Ok(())
}

#[tokio::test]
async fn test_command_serialization() -> TestResult {
    let command = TestDataBuilder::command()
        .with_type("SerializationTest")
        .with_payload(json!({"test": true, "number": 42}))
        .build();

    // Test that command can be serialized and deserialized
    let serialized = serde_json::to_string(&command.payload)?;
    let deserialized: serde_json::Value = serde_json::from_str(&serialized)?;

    TestAssertions::assert_json_equivalent(&command.payload, &deserialized)?;

    Ok(())
}

#[tokio::test]
async fn test_command_correlation_tracking() -> TestResult {
    let correlation_id = Uuid::new_v4();
    let command1 = TestDataBuilder::command().with_type("Command1").build();

    let command2 = TestDataBuilder::command().with_type("Command2").build();

    // Commands should have different IDs but can share correlation IDs
    assert_ne!(command1.id, command2.id);
    TestAssertions::assert_valid_uuid(&command1.correlation_id)?;
    TestAssertions::assert_valid_uuid(&command2.correlation_id)?;

    Ok(())
}

#[tokio::test]
async fn test_command_timestamp() -> TestResult {
    let before = chrono::Utc::now();
    let command = TestDataBuilder::command().build();
    let after = chrono::Utc::now();

    // Command creation time should be between before and after
    assert!(command.created_at >= before);
    assert!(command.created_at <= after);

    Ok(())
}

#[tokio::test]
async fn test_command_builder_pattern() -> TestResult {
    let user_id = Uuid::new_v4();
    let payload = json!({"action": "test", "data": [1, 2, 3]});

    let command = TestDataBuilder::command()
        .with_type("BuilderTest")
        .with_payload(payload.clone())
        .with_user(user_id)
        .build();

    assert_eq!(command.command_type, "BuilderTest");
    TestAssertions::assert_json_equivalent(&command.payload, &payload)?;
    assert_eq!(command.user_id, Some(user_id));

    Ok(())
}

#[tokio::test]
async fn test_command_validation() -> TestResult {
    // Test empty command type
    let command = TestDataBuilder::command().with_type("").build();

    assert!(command.command_type.is_empty());

    // Test null payload handling
    let command = TestDataBuilder::command()
        .with_payload(serde_json::Value::Null)
        .build();

    assert_eq!(command.payload, serde_json::Value::Null);

    Ok(())
}

#[tokio::test]
async fn test_command_complex_payload() -> TestResult {
    let complex_payload = json!({
        "pipeline": {
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "stages": [
                {
                    "name": "build",
                    "steps": ["compile", "test"]
                },
                {
                    "name": "deploy",
                    "steps": ["package", "upload"]
                }
            ]
        },
        "metadata": {
            "triggered_by": "webhook",
            "timestamp": "2024-01-01T10:00:00Z"
        }
    });

    let command = TestDataBuilder::command()
        .with_type("ComplexCommand")
        .with_payload(complex_payload.clone())
        .build();

    TestAssertions::assert_json_equivalent(&command.payload, &complex_payload)?;

    // Test nested field access
    let pipeline_id = command.payload["pipeline"]["id"].as_str();
    TestAssertions::assert_some(&pipeline_id)?;

    Ok(())
}
