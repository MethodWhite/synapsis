//! Terminal Writer - Direct User Input Zone Communication
//! 
//! Allows agents to write coordination messages directly to
//! user's terminal input zones when CLI/IDEs are waiting.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use anyhow::Result;

/// Terminal Writer for agent coordination
pub struct TerminalWriter {
    message_file: PathBuf,
}

impl TerminalWriter {
    pub fn new() -> Self {
        Self {
            message_file: PathBuf::from("/tmp/synapsis-agent-messages.txt"),
        }
    }
    
    /// Write coordination message to terminal input zone
    pub fn write_to_terminal(&self, message: &str, agent_id: &str) -> Result<()> {
        let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
        
        let formatted = format!(
            "\n╔══════════════════════════════════════════════════════════╗\n\
             ║  📡 SYNAPSIS AGENT COORDINATION MESSAGE                  ║\n\
             ╠══════════════════════════════════════════════════════════╣\n\
             ║  From: {:<52} ║\n\
             ║  Time: {:<52} ║\n\
             ╠══════════════════════════════════════════════════════════╣\n\
             ║  Message:\n\
             ║  {:<52} ║\n\
             ╚══════════════════════════════════════════════════════════╝\n\n\
             ⚠️  This is an automated coordination message.\n\
             💡  The agent is waiting for your confirmation.\n\n",
            agent_id,
            timestamp,
            Self::wrap_text(message, 52)
        );
        
        // Append to message file
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.message_file)?;
        
        file.write_all(formatted.as_bytes())?;
        file.flush()?;
        
        // Also print to stdout for immediate visibility
        print!("{}", formatted);
        std::io::stdout().flush()?;
        
        Ok(())
    }
    
    /// Write coordination prompt (for user action)
    pub fn write_prompt(&self, _prompt: &str, agent_id: &str, action_required: &str) -> Result<()> {
        let message = format!(
            "🔔 COORDINATION REQUIRED\n\n\
             Agent '{}' is waiting for:\n\
             {}\n\n\
             Please confirm when completed.",
            agent_id,
            action_required
        );
        
        self.write_to_terminal(&message, agent_id)?;
        
        // Also write to prompt-specific file
        let prompt_file = PathBuf::from("/tmp/synapsis-agent-prompt.txt");
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&prompt_file)?;
        
        file.write_all(format!("{}\n{}\n", agent_id, action_required).as_bytes())?;
        file.flush()?;
        
        Ok(())
    }
    
    /// Clear all agent messages
    pub fn clear_messages(&self) -> Result<()> {
        if self.message_file.exists() {
            std::fs::write(&self.message_file, "")?;
        }
        Ok(())
    }
    
    /// Get recent messages
    pub fn get_recent_messages(&self, lines: usize) -> Result<String> {
        if !self.message_file.exists() {
            return Ok("No messages".to_string());
        }
        
        let content = std::fs::read_to_string(&self.message_file)?;
        let recent: String = content
            .lines()
            .rev()
            .take(lines)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        
        Ok(recent)
    }
    
    fn wrap_text(text: &str, width: usize) -> String {
        text.chars()
            .collect::<Vec<_>>()
            .chunks(width)
            .map(|chunk| chunk.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n║  ")
    }
}

impl Default for TerminalWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_terminal_writer() {
        let writer = TerminalWriter::new();
        let result = writer.write_to_terminal("Test message", "test-agent");
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_wrap_text() {
        let wrapped = TerminalWriter::wrap_text("Hello World", 5);
        assert!(wrapped.contains("Hello"));
        assert!(wrapped.contains("World"));
    }
}
