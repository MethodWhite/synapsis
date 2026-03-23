//! Skills Registry Tests for Synapsis
//!
//! Unit tests for the SkillRegistry covering skill registration,
//! activation, deactivation, listing, and search functionality.

use std::env;
use synapsis::infrastructure::skills::{
    ActivationId, Skill, SkillActivation, SkillCategory, SkillId, SkillRegistry,
};

fn test_registry() -> SkillRegistry {
    env::set_var("XDG_DATA_HOME", "/tmp/synapsis-skills-test");
    std::fs::create_dir_all("/tmp/synapsis-skills-test/synapsis/skills").ok();
    let registry = SkillRegistry::new();
    registry.init().ok();
    registry
}

fn cleanup_test_dir() {
    std::fs::remove_dir_all("/tmp/synapsis-skills-test").ok();
}

mod skills_tests {
    use super::*;

    #[test]
    fn test_register_skill() {
        cleanup_test_dir();
        let registry = test_registry();

        let skill = Skill::new(
            "rust-coder".to_string(),
            "Expert in Rust programming".to_string(),
            SkillCategory::Coding,
        );

        let id = registry.register(skill);
        assert!(!id.0.is_empty(), "Skill ID should not be empty");

        let retrieved = registry.get(&id);
        assert!(retrieved.is_some(), "Skill should be retrievable");
        assert_eq!(retrieved.unwrap().name, "rust-coder");

        cleanup_test_dir();
    }

    #[test]
    fn test_register_multiple_skills() {
        cleanup_test_dir();
        let registry = test_registry();

        let skill1 = Skill::new(
            "skill1".to_string(),
            "Description 1".to_string(),
            SkillCategory::Coding,
        );
        let skill2 = Skill::new(
            "skill2".to_string(),
            "Description 2".to_string(),
            SkillCategory::Security,
        );
        let skill3 = Skill::new(
            "skill3".to_string(),
            "Description 3".to_string(),
            SkillCategory::Research,
        );

        registry.register(skill1);
        registry.register(skill2);
        registry.register(skill3);

        let count = registry.count();
        assert_eq!(count, 3, "Should have 3 registered skills");

        cleanup_test_dir();
    }

    #[test]
    fn test_register_with_tags() {
        cleanup_test_dir();
        let registry = test_registry();

        let skill = Skill::new(
            "api-designer".to_string(),
            "Design REST APIs".to_string(),
            SkillCategory::Coding,
        )
        .with_tags(vec![
            "rest".to_string(),
            "api".to_string(),
            "backend".to_string(),
        ]);

        registry.register(skill);

        let results = registry.search("api");
        assert!(!results.is_empty(), "Should find skill by tag");
        assert_eq!(results[0].name, "api-designer");

        cleanup_test_dir();
    }

    #[test]
    fn test_register_with_instructions() {
        cleanup_test_dir();
        let registry = test_registry();

        let skill = Skill::new(
            "test-skill".to_string(),
            "Testing skill".to_string(),
            SkillCategory::Testing,
        )
        .with_instructions("Follow these steps: 1. Plan 2. Execute 3. Verify");

        registry.register(skill);

        let by_name = registry.get_by_name("test-skill").unwrap();
        assert!(by_name.instructions.contains("steps"));

        cleanup_test_dir();
    }

    #[test]
    fn test_list_skills_by_category() {
        cleanup_test_dir();
        let registry = test_registry();

        registry.register(Skill::new(
            "coding1".to_string(),
            "Desc".to_string(),
            SkillCategory::Coding,
        ));
        registry.register(Skill::new(
            "coding2".to_string(),
            "Desc".to_string(),
            SkillCategory::Coding,
        ));
        registry.register(Skill::new(
            "security1".to_string(),
            "Desc".to_string(),
            SkillCategory::Security,
        ));

        let coding_skills = registry.list(Some(SkillCategory::Coding));
        let security_skills = registry.list(Some(SkillCategory::Security));
        let all_skills = registry.list(None);

        assert_eq!(coding_skills.len(), 2, "Should have 2 coding skills");
        assert_eq!(security_skills.len(), 1, "Should have 1 security skill");
        assert_eq!(all_skills.len(), 3, "Should have 3 total skills");

        cleanup_test_dir();
    }

    #[test]
    fn test_list_only_enabled_skills() {
        cleanup_test_dir();
        let registry = test_registry();

        let skill1 = Skill::new(
            "enabled-skill".to_string(),
            "Desc".to_string(),
            SkillCategory::Coding,
        );
        let skill2 = Skill::new(
            "disabled-skill".to_string(),
            "Desc".to_string(),
            SkillCategory::Coding,
        );

        let id1 = registry.register(skill1);
        let id2 = registry.register(skill2);

        registry.disable(&id2);

        let enabled = registry.list(None);
        assert_eq!(enabled.len(), 1, "Only enabled skills listed");
        assert_eq!(enabled[0].name, "enabled-skill");

        cleanup_test_dir();
    }

    #[test]
    fn test_enable_disable_skill() {
        cleanup_test_dir();
        let registry = test_registry();

        let skill = Skill::new(
            "toggle-skill".to_string(),
            "Desc".to_string(),
            SkillCategory::Coding,
        );
        let id = registry.register(skill);

        assert!(registry.disable(&id), "Should disable skill");
        assert!(!registry.enable(&id), "Should enable skill again");

        let skill_state = registry.get(&id).unwrap();
        assert!(skill_state.enabled, "Skill should be enabled after toggle");

        cleanup_test_dir();
    }

    #[test]
    fn test_activate_skill() {
        cleanup_test_dir();
        let registry = test_registry();

        let skill = Skill::new(
            "activatable".to_string(),
            "Can be activated".to_string(),
            SkillCategory::Coding,
        );
        let skill_id = registry.register(skill);

        let activation = registry.activate(
            &skill_id,
            Some("agent-1".to_string()),
            Some("session-1".to_string()),
            "context for activation",
        );

        assert!(activation.is_some(), "Activation should succeed");
        let act = activation.unwrap();
        assert_eq!(act.agent_id.as_deref(), Some("agent-1"));
        assert_eq!(act.session_id.as_deref(), Some("session-1"));
        assert!(act.success, "Activation should be successful");

        cleanup_test_dir();
    }

    #[test]
    fn test_deactivate_skill() {
        cleanup_test_dir();
        let registry = test_registry();

        let skill = Skill::new(
            "deactivatable".to_string(),
            "Can be deactivated".to_string(),
            SkillCategory::Coding,
        );
        let skill_id = registry.register(skill);

        let activation = registry.activate(&skill_id, None, None, "context").unwrap();

        assert!(
            activation.deactivated_at.is_none(),
            "Initially not deactivated"
        );

        registry.deactivate(&activation.id, true, None);

        let activations = registry.get_activations(10);
        let last = activations.first().unwrap();
        assert!(last.deactivated_at.is_some(), "Should be deactivated");
        assert!(last.success, "Should be marked as successful");

        cleanup_test_dir();
    }

    #[test]
    fn test_activate_nonexistent_skill() {
        cleanup_test_dir();
        let registry = test_registry();

        let fake_id = SkillId("nonexistent-id".to_string());
        let activation = registry.activate(&fake_id, None, None, "context");

        assert!(
            activation.is_none(),
            "Activation should fail for nonexistent skill"
        );

        cleanup_test_dir();
    }

    #[test]
    fn test_search_skills_by_name() {
        cleanup_test_dir();
        let registry = test_registry();

        registry.register(Skill::new(
            "rust-expert".to_string(),
            "Rust programming expert".to_string(),
            SkillCategory::Coding,
        ));
        registry.register(Skill::new(
            "python-developer".to_string(),
            "Python programming".to_string(),
            SkillCategory::Coding,
        ));
        registry.register(Skill::new(
            "rust-web-dev".to_string(),
            "Rust web development".to_string(),
            SkillCategory::Coding,
        ));

        let results = registry.search("rust");
        assert_eq!(results.len(), 2, "Should find 2 rust-related skills");

        cleanup_test_dir();
    }

    #[test]
    fn test_search_skills_by_description() {
        cleanup_test_dir();
        let registry = test_registry();

        registry.register(Skill::new(
            "skill1".to_string(),
            "Security audit expert".to_string(),
            SkillCategory::Security,
        ));
        registry.register(Skill::new(
            "skill2".to_string(),
            "Code reviewer".to_string(),
            SkillCategory::Coding,
        ));

        let results = registry.search("security");
        assert_eq!(results.len(), 1, "Should find skill by description");

        cleanup_test_dir();
    }

    #[test]
    fn test_search_skills_by_tags() {
        cleanup_test_dir();
        let registry = test_registry();

        let skill = Skill::new(
            "docker-expert".to_string(),
            "Container expert".to_string(),
            SkillCategory::DevOps,
        )
        .with_tags(vec!["containers".to_string(), "docker".to_string()]);

        registry.register(skill);

        let results = registry.search("containers");
        assert_eq!(results.len(), 1, "Should find by tag");

        cleanup_test_dir();
    }

    #[test]
    fn test_unregister_skill() {
        cleanup_test_dir();
        let registry = test_registry();

        let skill = Skill::new(
            "removable".to_string(),
            "Can be removed".to_string(),
            SkillCategory::Coding,
        );
        let id = registry.register(skill);

        assert_eq!(registry.count(), 1, "Should have 1 skill before removal");

        let removed = registry.unregister(&id);
        assert!(removed.is_some(), "Should return removed skill");
        assert_eq!(registry.count(), 0, "Should have 0 skills after removal");

        cleanup_test_dir();
    }

    #[test]
    fn test_get_by_name() {
        cleanup_test_dir();
        let registry = test_registry();

        let skill = Skill::new(
            "unique-skill-name".to_string(),
            "Unique description".to_string(),
            SkillCategory::Coding,
        );
        registry.register(skill);

        let found = registry.get_by_name("unique-skill-name");
        assert!(found.is_some(), "Should find skill by exact name");

        let not_found = registry.get_by_name("nonexistent");
        assert!(not_found.is_none(), "Should not find nonexistent skill");

        cleanup_test_dir();
    }

    #[test]
    fn test_get_active_count() {
        cleanup_test_dir();
        let registry = test_registry();

        registry.register(Skill::new(
            "skill1".to_string(),
            "Desc".to_string(),
            SkillCategory::Coding,
        ));
        let id2 = registry.register(Skill::new(
            "skill2".to_string(),
            "Desc".to_string(),
            SkillCategory::Coding,
        ));
        registry.register(Skill::new(
            "skill3".to_string(),
            "Desc".to_string(),
            SkillCategory::Coding,
        ));

        registry.disable(&id2);

        let active = registry.get_active_count();
        assert_eq!(active, 2, "Should have 2 active skills");

        cleanup_test_dir();
    }

    #[test]
    fn test_activation_tracking() {
        cleanup_test_dir();
        let registry = test_registry();

        let skill = Skill::new(
            "frequently-used".to_string(),
            "Used often".to_string(),
            SkillCategory::Coding,
        );
        let skill_id = registry.register(skill);

        for i in 0..5 {
            registry.activate(&skill_id, Some(format!("agent-{}", i)), None, "context");
        }

        let activations = registry.get_activations(10);
        assert_eq!(activations.len(), 5, "Should track all activations");

        cleanup_test_dir();
    }

    #[test]
    fn test_skill_category_from_str() {
        assert_eq!(SkillCategory::from_str("coding"), SkillCategory::Coding);
        assert_eq!(SkillCategory::from_str("security"), SkillCategory::Security);
        assert_eq!(SkillCategory::from_str("devops"), SkillCategory::DevOps);
        assert_eq!(SkillCategory::from_str("unknown"), SkillCategory::Custom);
        assert_eq!(SkillCategory::from_str("RUST"), SkillCategory::Custom);
    }

    #[test]
    fn test_skill_with_metadata() {
        cleanup_test_dir();
        let registry = test_registry();

        let mut skill = Skill::new(
            "meta-skill".to_string(),
            "Has metadata".to_string(),
            SkillCategory::Custom,
        );
        skill.author = Some("Test Author".to_string());
        skill.version = "2.0.0".to_string();

        let id = registry.register(skill);
        let retrieved = registry.get(&id).unwrap();

        assert_eq!(retrieved.author.as_deref(), Some("Test Author"));
        assert_eq!(retrieved.version, "2.0.0");

        cleanup_test_dir();
    }

    #[test]
    fn test_persistence_across_instances() {
        cleanup_test_dir();

        {
            let registry = test_registry();
            registry.register(Skill::new(
                "persisted-skill".to_string(),
                "Persists across instances".to_string(),
                SkillCategory::Coding,
            ));
        }

        {
            let registry = test_registry();
            let found = registry.get_by_name("persisted-skill");
            assert!(found.is_some(), "Skill should persist");
        }

        cleanup_test_dir();
    }
}
