//! Automatic Updater for Synapsis
//! 
//! Handles checking for updates on GitHub and performing self-updates.

use self_update::backends::github::Update;
use self_update::cargo_crate_version;
use anyhow::{Result, anyhow};
use std::sync::Arc;
use synapsis_core::infrastructure::database::Database;

pub struct AutoUpdater {
    pub repo_owner: String,
    pub repo_name: String,
    db: Arc<Database>,
}

impl AutoUpdater {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            repo_owner: "methodwhite".to_string(),
            repo_name: "synapsis".to_string(),
            db,
        }
    }

    /// Check if a newer version is available
    pub fn check_for_update(&self) -> Result<Option<String>> {
        let current_version = cargo_crate_version!();
        
        let releases = Update::configure()
            .repo_owner(&self.repo_owner)
            .repo_name(&self.repo_name)
            .bin_name("synapsis")
            .show_download_progress(false)
            .current_version(current_version)
            .build()?
            .get_latest_release()?;

        if self_update::version::bump_is_greater(current_version, &releases.version)? {
            Ok(Some(releases.version))
        } else {
            Ok(None)
        }
    }

    /// Perform the actual update
    pub fn perform_update(&self) -> Result<()> {
        let current_version = cargo_crate_version!();
        
        let status = Update::configure()
            .repo_owner(&self.repo_owner)
            .repo_name(&self.repo_name)
            .bin_name("synapsis")
            .show_download_progress(true)
            .current_version(current_version)
            .build()?
            .update()?;

        if status.updated() {
            println!("[Updater] Successfully updated to version {}", status.version());
            Ok(())
        } else {
            Err(anyhow!("Update failed or was already up to date"))
        }
    }

    /// Background check and notify
    pub fn check_and_notify(&self) {
        let repo_owner = self.repo_owner.clone();
        let repo_name = self.repo_name.clone();

        std::thread::spawn(move || {
            let current_version = cargo_crate_version!();
            let check = Update::configure()
                .repo_owner(&repo_owner)
                .repo_name(&repo_name)
                .bin_name("synapsis")
                .show_download_progress(false)
                .current_version(current_version)
                .build();

            if let Ok(update) = check {
                if let Ok(latest) = update.get_latest_release() {
                    if let Ok(true) = self_update::version::bump_is_greater(current_version, &latest.version) {
                        eprintln!("\n🚀 [Synapsis] A new version is available: v{} (current: v{})", latest.version, current_version);
                        eprintln!("   Run 'synapsis update' to install it automatically.\n");
                    }
                }
            }
        });
    }
}
