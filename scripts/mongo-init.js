// MongoDB initialization script for RustCI
db = db.getSiblingDB('rustci');

// Create collections
db.createCollection('users');
db.createCollection('workspaces');
db.createCollection('pipelines');
db.createCollection('jobs');
db.createCollection('runners');
db.createCollection('cluster_nodes');
db.createCollection('audit_logs');

// Create indexes for better performance
db.users.createIndex({ "email": 1 }, { unique: true });
db.users.createIndex({ "id": 1 }, { unique: true });

db.workspaces.createIndex({ "user_id": 1 });
db.workspaces.createIndex({ "id": 1 }, { unique: true });

db.pipelines.createIndex({ "workspace_id": 1 });
db.pipelines.createIndex({ "id": 1 }, { unique: true });
db.pipelines.createIndex({ "created_at": -1 });

db.jobs.createIndex({ "pipeline_id": 1 });
db.jobs.createIndex({ "id": 1 }, { unique: true });
db.jobs.createIndex({ "status": 1 });
db.jobs.createIndex({ "created_at": -1 });

db.runners.createIndex({ "id": 1 }, { unique: true });
db.runners.createIndex({ "node_id": 1 });
db.runners.createIndex({ "status": 1 });

db.cluster_nodes.createIndex({ "id": 1 }, { unique: true });
db.cluster_nodes.createIndex({ "status": 1 });
db.cluster_nodes.createIndex({ "last_heartbeat": -1 });

db.audit_logs.createIndex({ "timestamp": -1 });
db.audit_logs.createIndex({ "user_id": 1 });
db.audit_logs.createIndex({ "event_type": 1 });

print('RustCI database initialized successfully');