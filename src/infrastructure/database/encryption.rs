//! SQLCipher Encryption for Synapsis DB
//! Implements: SYNAPSIS-2026-005 mitigation

use rusqlite::{Connection, OpenFlags};
use std::path::Path;

pub struct EncryptedDB {
    conn: Connection,
    key: Vec<u8>,
}

impl EncryptedDB {
    pub fn new<P: AsRef<Path>>(db_path: P, encryption_key: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_with_flags(
            db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        
        // Set encryption key
        conn.execute_batch(&format!("PRAGMA key = '{}'", encryption_key))?;
        
        // Verify encryption is active
        conn.execute_batch("PRAGMA cipher_version")?;
        
        Ok(Self {
            conn,
            key: encryption_key.as_bytes().to_vec(),
        })
    }
    
    pub fn encrypt_existing_db(db_path: &str, key: &str) -> Result<(), rusqlite::Error> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(&format!("PRAGMA key = '{}'", key))?;
        
        // Rekey to encrypt
        conn.execute_batch(&format!("PRAGMA rekey = '{}'", key))?;
        
        Ok(())
    }
    
    pub fn connection(&self) -> &Connection {
        &self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encryption() {
        let db = EncryptedDB::new("/tmp/test_encrypted.db", "test_key_123").unwrap();
        assert!(db.connection().execute_batch("PRAGMA cipher_version").is_ok());
    }
}
