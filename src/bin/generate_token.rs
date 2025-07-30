use RustAutoDevOps::core::security::{JwtClaims, JwtManager, Role};
use std::collections::HashSet;

fn main() {
    let jwt_secret = "404E635266556A586E3272357538782F413F4428472B4B6250645367566B5970";
    let jwt_manager = JwtManager::new(jwt_secret.to_string(), 3600);
    
    let claims = JwtClaims::new(
        uuid::Uuid::new_v4(),
        "test@example.com".to_string(),
        vec![Role::Developer],
        3600,
    );
    
    match jwt_manager.create_token(&claims) {
        Ok(token) => {
            println!("Generated test JWT token:");
            println!("{}", token);
            println!("\nYou can now use this token in your curl command:");
            println!("curl --location 'http://localhost:8000/api/ci/pipelines/upload' \\");
            println!("--header 'Authorization: Bearer {}' \\", token);
            println!("--form 'pipeline=@\"/Users/ritabrataroychowdhury/Downloads/pipeline.yaml\"'");
        }
        Err(e) => {
            eprintln!("Failed to create token: {}", e);
        }
    }
}