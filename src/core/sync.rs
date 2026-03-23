//! Git Sync Module

use crate::domain::*;
use std::sync::RwLock;

const MANIFEST_VERSION: u32 = 1;
const MAX_CHUNK_SIZE: usize = 64 * 1024;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GitSyncConfig {
    pub repo_path: String,
    pub remote_url: Option<String>,
    pub branch: String,
    pub auto_commit: bool,
    pub commit_interval_secs: u64,
}

impl Default for GitSyncConfig {
    fn default() -> Self {
        Self {
            repo_path: ".synapsis".into(),
            remote_url: None,
            branch: "main".into(),
            auto_commit: true,
            commit_interval_secs: 300,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Manifest {
    pub id: ManifestId,
    pub version: u32,
    pub created_at: u64,
    pub updated_at: u64,
    pub agent_id: String,
    pub session_id: Option<String>,
    pub chunks: Vec<ChunkMetadata>,
    pub total_size: usize,
}

impl Manifest {
    pub fn new(agent_id: String, session_id: Option<String>) -> Self {
        let now = current_timestamp();
        Self {
            id: ManifestId::new(),
            version: MANIFEST_VERSION,
            created_at: now,
            updated_at: now,
            agent_id,
            session_id,
            chunks: Vec::new(),
            total_size: 0,
        }
    }
    pub fn add_chunk(&mut self, chunk_id: ChunkId, size: usize, hash: ContentHash) {
        self.chunks.push(ChunkMetadata {
            chunk_id,
            size,
            hash,
            index: self.chunks.len() as u32,
        });
        self.total_size += size;
        self.updated_at = current_timestamp();
    }
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub id: ChunkId,
    pub data: Vec<u8>,
}

impl Chunk {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            id: ChunkId::new(),
            data,
        }
    }
    pub fn size(&self) -> usize {
        self.data.len()
    }
}

pub struct GitSyncEngine {
    manifest: RwLock<Manifest>,
}

impl GitSyncEngine {
    pub fn new(config: GitSyncConfig) -> Self {
        Self {
            manifest: RwLock::new(Manifest::new(String::new(), None)),
        }
    }
    pub fn with_agent(agent_id: String, session_id: Option<String>, config: GitSyncConfig) -> Self {
        Self {
            manifest: RwLock::new(Manifest::new(agent_id, session_id)),
        }
    }
    pub fn sync_memory(&self, memory: &Memory) -> Result<ManifestId> {
        let chunks = self.chunk_content(&memory.content);
        let mut manifest = self.manifest.write().unwrap();
        manifest.agent_id = memory.agent_id.clone();
        manifest.session_id = memory.session_id.clone();
        manifest.chunks.clear();
        manifest.total_size = 0;
        for chunk in &chunks {
            manifest.add_chunk(chunk.id.clone(), chunk.size(), ContentHash::zero());
        }
        let manifest_id = manifest.id.clone();
        Ok(manifest_id)
    }
    fn chunk_content(&self, content: &str) -> Vec<Chunk> {
        let bytes = content.as_bytes();
        if bytes.len() <= MAX_CHUNK_SIZE {
            vec![Chunk::new(bytes.to_vec())]
        } else {
            bytes
                .chunks(MAX_CHUNK_SIZE)
                .map(|c| Chunk::new(c.to_vec()))
                .collect()
        }
    }
    pub fn get_sync_status(&self) -> SyncStatus {
        SyncStatus {
            last_sync: 0,
            commit_count: 0,
            pending_changes: false,
            conflict_detected: false,
            circuit_breaker_state: "closed".into(),
        }
    }
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
