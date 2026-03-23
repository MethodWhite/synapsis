//! Synapsis Skills Module
//!
//! Skill registry and management system for AI agents.
//! Skills are reusable capabilities that can be activated/deactivated.

use crate::core::uuid::Uuid;
use crate::domain::types::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: SkillId,
    pub name: String,
    pub description: String,
    pub category: SkillCategory,
    pub tags: Vec<String>,
    pub instructions: String,
    pub enabled: bool,
    pub version: String,
    pub author: Option<String>,
    pub scripts_path: Option<PathBuf>,
    pub examples_path: Option<PathBuf>,
    pub dependencies: Vec<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Skill {
    pub fn new(name: String, description: String, category: SkillCategory) -> Self {
        let now = Timestamp::now();
        Self {
            id: SkillId::new(),
            name,
            description,
            category,
            tags: Vec::new(),
            instructions: String::new(),
            enabled: true,
            version: "1.0.0".to_string(),
            author: None,
            scripts_path: None,
            examples_path: None,
            dependencies: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_instructions(mut self, instructions: &str) -> Self {
        self.instructions = instructions.to_string();
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum SkillCategory {
    Coding = 0,
    Research = 1,
    Design = 2,
    Communication = 3,
    Data = 4,
    Security = 5,
    DevOps = 6,
    Testing = 7,
    Documentation = 8,
    Custom = 9,
}

impl SkillCategory {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "coding" | "code" => Self::Coding,
            "research" => Self::Research,
            "design" => Self::Design,
            "communication" | "comms" => Self::Communication,
            "data" => Self::Data,
            "security" | "sec" => Self::Security,
            "devops" | "ops" => Self::DevOps,
            "testing" | "test" => Self::Testing,
            "docs" | "documentation" => Self::Documentation,
            _ => Self::Custom,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct SkillId(pub String);

impl SkillId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_hex_string())
    }
}

impl Default for SkillId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillActivation {
    pub id: ActivationId,
    pub skill_id: SkillId,
    pub agent_id: Option<String>,
    pub session_id: Option<String>,
    pub context: String,
    pub activated_at: Timestamp,
    pub deactivated_at: Option<Timestamp>,
    pub success: bool,
    pub error: Option<String>,
}

impl SkillActivation {
    pub fn new(skill_id: SkillId) -> Self {
        Self {
            id: ActivationId::new(),
            skill_id,
            agent_id: None,
            session_id: None,
            context: String::new(),
            activated_at: Timestamp::now(),
            deactivated_at: None,
            success: true,
            error: None,
        }
    }

    pub fn deactivate(&mut self, success: bool, error: Option<String>) {
        self.deactivated_at = Some(Timestamp::now());
        self.success = success;
        self.error = error;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct ActivationId(pub String);

impl ActivationId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_hex_string())
    }
}

impl Default for ActivationId {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SkillRegistry {
    skills: Arc<RwLock<HashMap<SkillId, Skill>>>,
    activations: Arc<RwLock<Vec<SkillActivation>>>,
    data_dir: PathBuf,
}

impl SkillRegistry {
    pub fn new() -> Self {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("synapsis")
            .join("skills");
        std::fs::create_dir_all(&data_dir).ok();

        Self {
            skills: Arc::new(RwLock::new(HashMap::new())),
            activations: Arc::new(RwLock::new(Vec::new())),
            data_dir,
        }
    }

    pub fn init(&self) -> std::io::Result<()> {
        self.load()?;
        Ok(())
    }

    pub fn load(&self) -> std::io::Result<()> {
        let skills_file = self.data_dir.join("skills.json");
        if skills_file.exists() {
            if let Ok(data) = std::fs::read_to_string(&skills_file) {
                if let Ok(skills) = serde_json::from_str::<HashMap<SkillId, Skill>>(&data) {
                    *self.skills.write().unwrap() = skills;
                }
            }
        }

        let activations_file = self.data_dir.join("activations.json");
        if activations_file.exists() {
            if let Ok(data) = std::fs::read_to_string(&activations_file) {
                if let Ok(acts) = serde_json::from_str::<Vec<SkillActivation>>(&data) {
                    *self.activations.write().unwrap() = acts;
                }
            }
        }

        Ok(())
    }

    pub fn save(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;

        let skills_file = self.data_dir.join("skills.json");
        let skills = self.skills.read().unwrap();
        let data = serde_json::to_string_pretty(&*skills)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(skills_file, data)?;

        let activations_file = self.data_dir.join("activations.json");
        let activations = self.activations.read().unwrap();
        let data = serde_json::to_string_pretty(&*activations)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(activations_file, data)?;

        Ok(())
    }

    pub fn register(&self, skill: Skill) -> SkillId {
        let id = skill.id.clone();
        self.skills.write().unwrap().insert(id.clone(), skill);
        let _ = self.save();
        id
    }

    pub fn unregister(&self, id: &SkillId) -> Option<Skill> {
        let skill = self.skills.write().unwrap().remove(id);
        let _ = self.save();
        skill
    }

    pub fn get(&self, id: &SkillId) -> Option<Skill> {
        self.skills.read().unwrap().get(id).cloned()
    }

    pub fn get_by_name(&self, name: &str) -> Option<Skill> {
        self.skills
            .read()
            .unwrap()
            .values()
            .find(|s| s.name == name && s.enabled)
            .cloned()
    }

    pub fn list(&self, category: Option<SkillCategory>) -> Vec<Skill> {
        let skills = self.skills.read().unwrap();
        match category {
            Some(cat) => skills
                .values()
                .filter(|s| s.category == cat && s.enabled)
                .cloned()
                .collect(),
            None => skills.values().filter(|s| s.enabled).cloned().collect(),
        }
    }

    pub fn search(&self, query: &str) -> Vec<Skill> {
        let q = query.to_lowercase();
        self.skills
            .read()
            .unwrap()
            .values()
            .filter(|s| {
                s.enabled
                    && (s.name.to_lowercase().contains(&q)
                        || s.description.to_lowercase().contains(&q)
                        || s.tags.iter().any(|t| t.to_lowercase().contains(&q)))
            })
            .cloned()
            .collect()
    }

    pub fn enable(&self, id: &SkillId) -> bool {
        if let Some(skill) = self.skills.write().unwrap().get_mut(id) {
            skill.enabled = true;
            skill.updated_at = Timestamp::now();
            let _ = self.save();
            true
        } else {
            false
        }
    }

    pub fn disable(&self, id: &SkillId) -> bool {
        if let Some(skill) = self.skills.write().unwrap().get_mut(id) {
            skill.enabled = false;
            skill.updated_at = Timestamp::now();
            let _ = self.save();
            true
        } else {
            false
        }
    }

    pub fn activate(
        &self,
        skill_id: &SkillId,
        agent_id: Option<String>,
        session_id: Option<String>,
        context: &str,
    ) -> Option<SkillActivation> {
        if !self.skills.read().unwrap().contains_key(skill_id) {
            return None;
        }

        let mut activation = SkillActivation::new(skill_id.clone());
        activation.agent_id = agent_id;
        activation.session_id = session_id;
        activation.context = context.to_string();

        self.activations.write().unwrap().push(activation.clone());
        let _ = self.save();

        Some(activation)
    }

    pub fn deactivate(
        &self,
        activation_id: &ActivationId,
        success: bool,
        error: Option<String>,
    ) -> bool {
        let mut activations = self.activations.write().unwrap();
        if let Some(act) = activations.iter_mut().find(|a| a.id == *activation_id) {
            act.deactivate(success, error);
            drop(activations);
            let _ = self.save();
            true
        } else {
            false
        }
    }

    pub fn get_activations(&self, limit: usize) -> Vec<SkillActivation> {
        let activations = self.activations.read().unwrap();
        activations.iter().rev().take(limit).cloned().collect()
    }

    pub fn get_active_count(&self) -> usize {
        self.skills
            .read()
            .unwrap()
            .values()
            .filter(|s| s.enabled)
            .count()
    }

    pub fn count(&self) -> usize {
        self.skills.read().unwrap().len()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for SkillRegistry {
    fn clone(&self) -> Self {
        Self {
            skills: self.skills.clone(),
            activations: self.activations.clone(),
            data_dir: self.data_dir.clone(),
        }
    }
}

impl SkillRegistry {
    pub fn register_default_skills(&self) {
        let default_skills = vec![
            Skill::new(
                "debugger".to_string(),
                "Debugging and troubleshooting skill for diagnosing errors, analyzing stack traces, and finding root causes".to_string(),
                SkillCategory::Coding,
            )
            .with_tags(vec!["debug".to_string(), "troubleshoot".to_string(), "error".to_string(), "diagnostic".to_string()])
            .with_instructions(r#"
## Debugger Skill
Use systematic debugging methodology:
1. **Reproduce** - Get exact error message and conditions
2. **Isolate** - Find the minimal code that triggers the issue
3. **Hypothesize** - Generate possible causes
4. **Test** - Verify each hypothesis
5. **Fix** - Apply minimal fix
6. **Verify** - Confirm fix works and doesn't break other tests

Tools: print debugging, logging, breakpoints, stack traces, profiler output
"#),
            Skill::new(
                "developer".to_string(),
                "Full-stack development skill for writing, reviewing, and refactoring code with best practices".to_string(),
                SkillCategory::Coding,
            )
            .with_tags(vec!["code".to_string(), "refactor".to_string(), "review".to_string(), "architecture".to_string()])
            .with_instructions(r#"
## Developer Skill
Follow clean code principles:
1. **Readability** - Clear naming, comments where needed
2. **Simplicity** - Prefer simple solutions over clever ones
3. **Modularity** - Single responsibility, low coupling
4. **Testability** - Code that's easy to test
5. **Documentation** - Docs for public APIs and complex logic

Workflow: Design → Implement → Test → Review → Refactor
"#),
            Skill::new(
                "qa".to_string(),
                "Quality Assurance skill for testing strategies, test case design, and defect analysis".to_string(),
                SkillCategory::Testing,
            )
            .with_tags(vec!["test".to_string(), "quality".to_string(), "qa".to_string(), "validation".to_string()])
            .with_instructions(r#"
## QA Skill
Testing pyramid approach:
1. **Unit tests** - Fast, isolated, many
2. **Integration tests** - Component interaction
3. **E2E tests** - Critical user flows only

Test design principles:
- Given-When-Then format for test cases
- Cover happy path and edge cases
- Test boundaries and invalid inputs
- Automate repetitive testing

Defect analysis: severity, reproducibility, root cause
"#),
            Skill::new(
                "pentester".to_string(),
                "Offensive security testing skill for vulnerability assessment and penetration testing".to_string(),
                SkillCategory::Security,
            )
            .with_tags(vec!["security".to_string(), "pentest".to_string(), "vulnerability".to_string(), "offensive".to_string()])
            .with_instructions(r#"
## Pentester Skill
Security testing methodology (OWASP style):
1. **Recon** - Information gathering
2. **Threat modeling** - Identify attack surfaces
3. **Vulnerability analysis** - Find weaknesses
4. **Exploitation** - Prove exploitability (authorized only)
5. **Reporting** - Document findings with PoC

Focus areas:
- Injection (SQL, XSS, Command)
- Authentication/Authorization flaws
- Sensitive data exposure
- Security misconfiguration
- API security

Always have authorization before testing!
"#),
            Skill::new(
                "blueteamer".to_string(),
                "Defensive security skill for threat detection, incident response, and security hardening".to_string(),
                SkillCategory::Security,
            )
            .with_tags(vec!["security".to_string(), "defense".to_string(), "incident".to_string(), "hardening".to_string()])
            .with_instructions(r#"
## Blueteamer Skill
Defense in depth strategy:
1. **Prevention** - Hardening, patching, configs
2. **Detection** - Logging, monitoring, alerting
3. **Response** - Incident handling, forensics
4. **Recovery** - Business continuity, backup

Key areas:
- SIEM rules and correlation
- EDR/XDR detection patterns
- Secure configuration (CIS benchmarks)
- Vulnerability management
- Security monitoring

Threat hunting: Hypothesis → Data collection → Analysis → Response
"#),
            Skill::new(
                "architect".to_string(),
                "System architecture skill for designing scalable, maintainable, and secure systems".to_string(),
                SkillCategory::Coding,
            )
            .with_tags(vec!["architecture".to_string(), "design".to_string(), "system".to_string(), "scalability".to_string()])
            .with_instructions(r#"
## Architect Skill
Architecture principles:
1. **Requirements** - Functional and non-functional
2. **Patterns** - Choose appropriate patterns
3. **Trade-offs** - Balance competing concerns
4. **Documentation** - ADRs, diagrams, decisions

Key considerations:
- Scalability (horizontal vs vertical)
- Availability (SLA targets)
- Consistency vs Availability (CAP)
- Security by design
- Cost optimization

Review: Async, DR, Failure modes, Operational excellence
"#),
            Skill::new(
                "devops".to_string(),
                "DevOps skill for CI/CD pipelines, infrastructure as code, and deployment automation".to_string(),
                SkillCategory::DevOps,
            )
            .with_tags(vec!["ci/cd".to_string(), "automation".to_string(), "infrastructure".to_string(), "deployment".to_string()])
            .with_instructions(r#"
## DevOps Skill
Pipeline best practices:
1. **Version control** - All things in git
2. **Automation** - Build, test, deploy scripts
3. **Testing** - Automated at each stage
4. **Monitoring** - Deploy with observability
5. **Rollback** - Always have a way back

Infrastructure as Code:
- Idempotent configurations
- State management
- Secret handling
- Environment parity

SRE principles: SLIs/SLOs/SLAs, error budgets, SLO-based alerting
"#),
            Skill::new(
                "data-engineer".to_string(),
                "Data engineering skill for pipelines, ETL, data modeling, and analytics infrastructure".to_string(),
                SkillCategory::Data,
            )
            .with_tags(vec!["data".to_string(), "pipeline".to_string(), "etl".to_string(), "analytics".to_string()])
            .with_instructions(r#"
## Data Engineer Skill
Data pipeline design:
1. **Ingestion** - Batch vs streaming sources
2. **Processing** - Transform, clean, aggregate
3. **Storage** - DWH, data lake, feature store
4. **Serving** - APIs, dashboards, reports

Data quality:
- Validation at ingestion
- Schema enforcement
- Lineage tracking
- Anomaly detection

Performance: Partitioning, indexing, compression, query optimization
"#),
        ];

        for skill in default_skills {
            if self.get_by_name(&skill.name).is_none() {
                self.register(skill);
            }
        }
    }
}

mod dirs {
    use std::path::PathBuf;
    pub fn data_local_dir() -> Option<PathBuf> {
        std::env::var("XDG_DATA_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".local/share"))
            })
            .or_else(|| std::env::var("APPDATA").ok().map(PathBuf::from))
    }
}
