//! Synapsis Recycle Bin
//!
//! Stores recycled messages with intelligent categorization and search.

use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use super::categorizer::{
    MessageCategory, MessageMetadata, SmartCategorizer,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecycledEntry {
    pub id: String,
    pub encrypted_content: Vec<u8>,
    pub category: MessageCategory,
    pub keywords: Vec<String>,
    pub task_id: Option<String>,
    pub agent_fingerprint: String,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub access_count: u32,
    pub size_bytes: usize,
    pub extra_encrypted: bool,
    pub metadata: Option<MessageMetadata>,
}

impl RecycledEntry {
    pub fn is_expired(&self, now: i64) -> bool {
        match self.expires_at {
            Some(expiry) => now > expiry,
            None => false,
        }
    }

    pub fn should_never_delete(&self) -> bool {
        self.category.should_never_delete()
    }
}

#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub keywords: Vec<String>,
    pub category: Option<MessageCategory>,
    pub task_id: Option<String>,
    pub agent_fingerprint: Option<String>,
    pub from_time: Option<i64>,
    pub to_time: Option<i64>,
    pub limit: usize,
    pub offset: usize,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            keywords: Vec::new(),
            category: None,
            task_id: None,
            agent_fingerprint: None,
            from_time: None,
            to_time: None,
            limit: 50,
            offset: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub entries: Vec<RecycledEntry>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecycleStats {
    pub total_entries: usize,
    pub by_category: HashMap<String, usize>,
    pub by_agent: HashMap<String, usize>,
    pub total_size_bytes: usize,
    pub oldest_entry: Option<i64>,
    pub newest_entry: Option<i64>,
    pub expired_entries: usize,
}

pub struct RecycleBin {
    entries: Arc<RwLock<HashMap<String, RecycledEntry>>>,
    categorizer: SmartCategorizer,
    by_keyword: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    by_category: Arc<RwLock<HashMap<MessageCategory, HashSet<String>>>>,
    by_task: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    by_agent: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    by_time: Arc<RwLock<BinaryHeap<(i64, String)>>>,
    data_dir: PathBuf,
}

impl RecycleBin {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            categorizer: SmartCategorizer::new(),
            by_keyword: Arc::new(RwLock::new(HashMap::new())),
            by_category: Arc::new(RwLock::new(HashMap::new())),
            by_task: Arc::new(RwLock::new(HashMap::new())),
            by_agent: Arc::new(RwLock::new(HashMap::new())),
            by_time: Arc::new(RwLock::new(BinaryHeap::new())),
            data_dir,
        }
    }

    pub fn load(&self) -> Result<(), std::io::Error> {
        let bin_path = self.data_dir.join("recycle_bin.json");

        if bin_path.exists() {
            let data = std::fs::read_to_string(&bin_path)?;
            if let Ok(entries) = serde_json::from_str::<HashMap<String, RecycledEntry>>(&data) {
                let mut entries_guard = self.entries.write().unwrap();
                *entries_guard = entries;

                self.rebuild_indexes()?;
            }
        }

        Ok(())
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let bin_path = self.data_dir.join("recycle_bin.json");
        let entries = self.entries.read().unwrap();
        let data = serde_json::to_string_pretty(&*entries)?;
        std::fs::write(bin_path, data)
    }

    fn rebuild_indexes(&self) -> Result<(), std::io::Error> {
        let entries = self.entries.read().unwrap();

        {
            let mut by_keyword = self.by_keyword.write().unwrap();
            *by_keyword = HashMap::new();
        }
        {
            let mut by_category = self.by_category.write().unwrap();
            *by_category = HashMap::new();
        }
        {
            let mut by_task = self.by_task.write().unwrap();
            *by_task = HashMap::new();
        }
        {
            let mut by_agent = self.by_agent.write().unwrap();
            *by_agent = HashMap::new();
        }
        {
            let mut by_time = self.by_time.write().unwrap();
            *by_time = BinaryHeap::new();
        }

        for (id, entry) in entries.iter() {
            self.add_to_indexes_internal(id, entry)?;
        }

        Ok(())
    }

    fn add_to_indexes_internal(
        &self,
        id: &str,
        entry: &RecycledEntry,
    ) -> Result<(), std::io::Error> {
        {
            let mut by_keyword = self.by_keyword.write().unwrap();
            for keyword in &entry.keywords {
                by_keyword
                    .entry(keyword.clone())
                    .or_insert_with(HashSet::new)
                    .insert(id.to_string());
            }
        }

        {
            let mut by_category = self.by_category.write().unwrap();
            by_category
                .entry(entry.category)
                .or_insert_with(HashSet::new)
                .insert(id.to_string());
        }

        if let Some(ref task_id) = entry.task_id {
            let mut by_task = self.by_task.write().unwrap();
            by_task
                .entry(task_id.clone())
                .or_insert_with(HashSet::new)
                .insert(id.to_string());
        }

        {
            let mut by_agent = self.by_agent.write().unwrap();
            by_agent
                .entry(entry.agent_fingerprint.clone())
                .or_insert_with(HashSet::new)
                .insert(id.to_string());
        }

        {
            let mut by_time = self.by_time.write().unwrap();
            by_time.push((entry.created_at, id.to_string()));
        }

        Ok(())
    }

    fn add_to_indexes(&self, id: &str, entry: &RecycledEntry) -> Result<(), std::io::Error> {
        self.add_to_indexes_internal(id, entry)
    }

    fn remove_from_indexes(&self, id: &str, entry: &RecycledEntry) {
        {
            let mut by_keyword = self.by_keyword.write().unwrap();
            for keyword in &entry.keywords {
                if let Some(ids) = by_keyword.get_mut(keyword) {
                    ids.remove(id);
                    if ids.is_empty() {
                        by_keyword.remove(keyword);
                    }
                }
            }
        }

        {
            let mut by_category = self.by_category.write().unwrap();
            if let Some(ids) = by_category.get_mut(&entry.category) {
                ids.remove(id);
            }
        }

        if let Some(ref task_id) = entry.task_id {
            let mut by_task = self.by_task.write().unwrap();
            if let Some(ids) = by_task.get_mut(task_id) {
                ids.remove(id);
            }
        }

        {
            let mut by_agent = self.by_agent.write().unwrap();
            if let Some(ids) = by_agent.get_mut(&entry.agent_fingerprint) {
                ids.remove(id);
            }
        }
    }

    pub fn store(
        &self,
        content: &[u8],
        metadata: Option<MessageMetadata>,
        agent_fingerprint: &str,
    ) -> Result<String, RecycleError> {
        let id = generate_id();
        let now = current_timestamp();

        let categorization = if let Some(ref meta) = metadata {
            if let Some(ref method) = meta.method {
                self.categorizer.categorize(method, metadata.as_ref())
            } else {
                self.categorizer.categorize("", metadata.as_ref())
            }
        } else {
            self.categorizer.categorize("", None)
        };

        let expires_at = match categorization.category.ttl_seconds() {
            Some(ttl) if ttl > 0 => Some(now + ttl),
            _ => None,
        };

        let entry = RecycledEntry {
            id: id.clone(),
            encrypted_content: content.to_vec(),
            category: categorization.category,
            keywords: categorization.keywords.clone(),
            task_id: metadata.as_ref().and_then(|m| m.task_id.clone()),
            agent_fingerprint: agent_fingerprint.to_string(),
            created_at: now,
            expires_at,
            access_count: 0,
            size_bytes: content.len(),
            extra_encrypted: categorization.category.requires_extra_encryption(),
            metadata,
        };

        {
            let mut entries = self.entries.write().unwrap();
            entries.insert(id.clone(), entry.clone());
        }

        if let Err(e) = self.add_to_indexes(&id, &entry) {
            eprintln!("Warning: Failed to update indexes: {}", e);
        }

        if let Err(e) = self.save() {
            eprintln!("Warning: Failed to save recycle bin: {}", e);
        }

        Ok(id)
    }

    pub fn get(&self, id: &str) -> Option<RecycledEntry> {
        let mut entries = self.entries.write().unwrap();

        if let Some(entry) = entries.get_mut(id) {
            entry.access_count += 1;
            Some(entry.clone())
        } else {
            None
        }
    }

    pub fn search(&self, query: &SearchQuery) -> SearchResult {
        let mut candidate_ids: HashSet<String> = HashSet::new();
        let mut first = true;

        for keyword in &query.keywords {
            let by_keyword = self.by_keyword.read().unwrap();
            if let Some(ids) = by_keyword.get(keyword) {
                if first {
                    candidate_ids = ids.clone();
                    first = false;
                } else {
                    candidate_ids.retain(|id| ids.contains(id));
                }
            } else if !first {
                return SearchResult {
                    entries: Vec::new(),
                    total: 0,
                    offset: query.offset,
                    limit: query.limit,
                };
            }
        }

        if query.keywords.is_empty() {
            let entries = self.entries.read().unwrap();
            candidate_ids = entries.keys().cloned().collect();
        }

        if let Some(ref task_id) = query.task_id {
            let by_task = self.by_task.read().unwrap();
            if let Some(ids) = by_task.get(task_id) {
                if first {
                    candidate_ids = ids.clone();
                    first = false;
                } else {
                    candidate_ids.retain(|id| ids.contains(id));
                }
            }
        }

        if let Some(ref fingerprint) = query.agent_fingerprint {
            let by_agent = self.by_agent.read().unwrap();
            if let Some(ids) = by_agent.get(fingerprint) {
                if first {
                    candidate_ids = ids.clone();
                } else {
                    candidate_ids.retain(|id| ids.contains(id));
                }
            }
        }

        let entries = self.entries.read().unwrap();
        let now = current_timestamp();

        let mut results: Vec<RecycledEntry> = entries
            .values()
            .filter(|e| candidate_ids.contains(&e.id))
            .filter(|e| {
                if e.is_expired(now) {
                    return false;
                }

                if let Some(ref cat) = query.category {
                    if &e.category != cat {
                        return false;
                    }
                }

                if let Some(from) = query.from_time {
                    if e.created_at < from {
                        return false;
                    }
                }

                if let Some(to) = query.to_time {
                    if e.created_at > to {
                        return false;
                    }
                }

                true
            })
            .cloned()
            .collect();

        results.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        let total = results.len();

        let offset = query.offset.min(total);
        let limit = query.limit.min(1000);
        results = results.into_iter().skip(offset).take(limit).collect();

        SearchResult {
            entries: results,
            total,
            offset,
            limit,
        }
    }

    pub fn delete(&self, id: &str) -> bool {
        let entry = {
            let mut entries = self.entries.write().unwrap();
            entries.remove(id)
        };

        if let Some(e) = entry {
            self.remove_from_indexes(id, &e);
            let _ = self.save();
            true
        } else {
            false
        }
    }

    pub fn cleanup_expired(&self) -> usize {
        let now = current_timestamp();
        let mut deleted = 0;

        let ids_to_delete: Vec<String> = {
            let entries = self.entries.read().unwrap();
            entries
                .iter()
                .filter(|(_, e)| e.is_expired(now) && !e.should_never_delete())
                .map(|(id, _)| id.clone())
                .collect()
        };

        for id in ids_to_delete {
            if self.delete(&id) {
                deleted += 1;
            }
        }

        deleted
    }

    pub fn cleanup_category(&self, category: MessageCategory) -> usize {
        let ids: Vec<String> = {
            let by_category = self.by_category.read().unwrap();
            by_category
                .get(&category)
                .map(|ids| ids.iter().cloned().collect())
                .unwrap_or_default()
        };

        let mut deleted = 0;
        for id in ids {
            if self.delete(&id) {
                deleted += 1;
            }
        }

        deleted
    }

    pub fn stats(&self) -> RecycleStats {
        let entries = self.entries.read().unwrap();
        let now = current_timestamp();

        let mut by_category = HashMap::new();
        let mut by_agent = HashMap::new();
        let mut total_size = 0;
        let mut oldest = None;
        let mut newest = None;
        let mut expired = 0;

        for entry in entries.values() {
            *by_category
                .entry(entry.category.as_str().to_string())
                .or_insert(0) += 1;
            *by_agent.entry(entry.agent_fingerprint.clone()).or_insert(0) += 1;
            total_size += entry.size_bytes;

            match oldest {
                None => oldest = Some(entry.created_at),
                Some(t) if entry.created_at < t => oldest = Some(entry.created_at),
                _ => {}
            }

            match newest {
                None => newest = Some(entry.created_at),
                Some(t) if entry.created_at > t => newest = Some(entry.created_at),
                _ => {}
            }

            if entry.is_expired(now) {
                expired += 1;
            }
        }

        RecycleStats {
            total_entries: entries.len(),
            by_category,
            by_agent,
            total_size_bytes: total_size,
            oldest_entry: oldest,
            newest_entry: newest,
            expired_entries: expired,
        }
    }

    pub fn get_categorizer(&self) -> &SmartCategorizer {
        &self.categorizer
    }

    pub fn clear(&self) -> usize {
        let count = {
            let entries = self.entries.read().unwrap();
            entries.len()
        };

        let mut entries = self.entries.write().unwrap();
        entries.clear();

        {
            let mut by_keyword = self.by_keyword.write().unwrap();
            by_keyword.clear();
        }
        {
            let mut by_category = self.by_category.write().unwrap();
            by_category.clear();
        }
        {
            let mut by_task = self.by_task.write().unwrap();
            by_task.clear();
        }
        {
            let mut by_agent = self.by_agent.write().unwrap();
            by_agent.clear();
        }
        {
            let mut by_time = self.by_time.write().unwrap();
            *by_time = BinaryHeap::new();
        }

        let _ = self.save();

        count
    }
}

#[derive(Debug, Clone)]
pub enum RecycleError {
    NotFound,
    StorageError(String),
    EncryptionError,
}

impl std::fmt::Display for RecycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecycleError::NotFound => write!(f, "Entry not found"),
            RecycleError::StorageError(e) => write!(f, "Storage error: {}", e),
            RecycleError::EncryptionError => write!(f, "Encryption error"),
        }
    }
}

impl std::error::Error for RecycleError {}

fn generate_id() -> String {
    let mut id = vec![0u8; 16];
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);

    for (i, byte) in id.iter_mut().enumerate() {
        let val = seed.wrapping_mul(i as u64 + 1).wrapping_mul(1103515245);
        *byte = ((val >> 16) ^ val) as u8;
    }

    hex_encode(&id)
}

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}

fn current_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_and_get() {
        let bin = RecycleBin::new(PathBuf::from("/tmp/test_synapsis"));

        let id = bin.store(b"test content", None, "agent-1").unwrap();

        let entry = bin.get(&id).unwrap();
        assert_eq!(entry.encrypted_content, b"test content");
        assert_eq!(entry.agent_fingerprint, "agent-1");
    }

    #[test]
    fn test_search_by_keyword() {
        let bin = RecycleBin::new(PathBuf::from("/tmp/test_synapsis"));

        bin.store(
            b"task message",
            Some(MessageMetadata {
                task_id: Some("task-123".to_string()),
                ..Default::default()
            }),
            "agent-1",
        )
        .unwrap();

        bin.store(b"other message", None, "agent-2").unwrap();

        let results = bin.search(&SearchQuery {
            keywords: vec!["task".to_string()],
            ..Default::default()
        });

        assert_eq!(results.total, 1);
    }

    #[test]
    fn test_search_by_task() {
        let bin = RecycleBin::new(PathBuf::from("/tmp/test_synapsis"));

        bin.store(
            b"task 1",
            Some(MessageMetadata {
                task_id: Some("task-abc".to_string()),
                ..Default::default()
            }),
            "agent-1",
        )
        .unwrap();

        bin.store(
            b"task 2",
            Some(MessageMetadata {
                task_id: Some("task-xyz".to_string()),
                ..Default::default()
            }),
            "agent-2",
        )
        .unwrap();

        let results = bin.search(&SearchQuery {
            task_id: Some("task-abc".to_string()),
            ..Default::default()
        });

        assert_eq!(results.total, 1);
    }

    #[test]
    fn test_stats() {
        let bin = RecycleBin::new(PathBuf::from("/tmp/test_synapsis"));

        bin.store(b"content 1", None, "agent-1").unwrap();
        bin.store(b"content 2", None, "agent-2").unwrap();

        let stats = bin.stats();

        assert_eq!(stats.total_entries, 2);
        assert_eq!(stats.by_agent.get("agent-1"), Some(&1));
        assert_eq!(stats.by_agent.get("agent-2"), Some(&1));
    }

    #[test]
    fn test_delete() {
        let bin = RecycleBin::new(PathBuf::from("/tmp/test_synapsis"));

        let id = bin.store(b"test", None, "agent-1").unwrap();

        assert!(bin.get(&id).is_some());

        bin.delete(&id);

        assert!(bin.get(&id).is_none());
    }
}
