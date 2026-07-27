//! Crate-dependency assertions — `N6`, and the "zero deps" claim for `treepo-det`.
//!
//! Architecture D1, the document's own "single most load-bearing decision":
//!
//! > "With Grow in a crate that has no Bevy types at all, the violation does not compile."
//!
//! `N6` says structural work must never enter the continuous loop. The mechanism is that
//! the crates doing structural work cannot see a `World`, because they cannot see `bevy`.
//! That is a real compile-time guarantee — but only for as long as nobody adds the
//! dependency. This command is what notices if somebody does.
//!
//! It is deliberately not a lint or a code-review convention. It is one shell command with
//! an exit code, run by CI on every push (campaign Standing Rules).

use std::process::Command;

/// The generative set, quoted from the campaign's Standing Rules. None of these may reach
/// `bevy`, at any depth, under any feature combination.
///
/// Crates from later phases are listed now and skipped until they exist, so that adding
/// one does not also require remembering to add it here.
const GENERATIVE_CRATES: &[(&str, &str)] = &[
    ("treepo-det", "Phase 0"),
    ("treepo-model", "Phase 1"),
    ("treepo-vcs", "Phase 1"),
    ("treepo-store", "Phase 2"),
    ("treepo-id", "Phase 4"),
    ("treepo-gen", "Phase 3"),
    ("treepo-grow", "Phase 6"),
    ("treepo-export", "Phase 9"),
];

pub(crate) fn run(_args: &[String]) -> Result<(), String> {
    let root = crate::workspace_root();
    println!("dep-guard — no generative crate may depend on bevy (N6)\n");

    let mut present = 0usize;
    let mut violations = Vec::new();

    for (crate_name, phase) in GENERATIVE_CRATES {
        if !root
            .join("crates")
            .join(crate_name)
            .join("Cargo.toml")
            .exists()
        {
            println!("  {crate_name:<16} absent — lands in {phase}");
            continue;
        }
        present += 1;

        let packages = dependency_packages(crate_name)?;
        let bevy: Vec<&String> = packages
            .iter()
            .filter(|name| *name == "bevy" || name.starts_with("bevy_"))
            .collect();

        if bevy.is_empty() {
            println!(
                "  {crate_name:<16} clean ({} packages in graph)",
                packages.len()
            );
        } else {
            println!("  {crate_name:<16} VIOLATION");
            for name in &bevy {
                violations.push(format!("{crate_name} depends on {name}"));
            }
        }

        // treepo-det carries a stronger claim than the rest of the set: the architecture
        // calls it "zero deps beyond core", and it is the root of the dependency graph.
        // A single external crate here would put something outside our control underneath
        // every generated value in the product.
        if *crate_name == "treepo-det" && packages.len() != 1 {
            let extra: Vec<&String> = packages.iter().filter(|n| *n != "treepo-det").collect();
            violations.push(format!(
                "treepo-det must have zero dependencies, found: {extra:?}"
            ));
        }
    }

    if present == 0 {
        return Err("no generative crate found — is the workspace laid out correctly?".to_owned());
    }

    if violations.is_empty() {
        println!("\n{present} generative crate(s) checked, all clean");
        Ok(())
    } else {
        for violation in &violations {
            eprintln!("  ! {violation}");
        }
        Err(format!(
            "{} dependency violation(s). N6 is enforced by the dependency graph — \
             if a generative crate needs bevy, the phase boundary (architecture D1) has \
             moved and that is a decision, not a dependency edit.",
            violations.len()
        ))
    }
}

/// Every package in a crate's normal + build dependency graph.
///
/// `--all-features` is the point: it asks whether *any* feature combination can reach
/// `bevy`, not merely whether the default one does.
fn dependency_packages(crate_name: &str) -> Result<Vec<String>, String> {
    let output = Command::new(crate::cargo())
        .current_dir(crate::workspace_root())
        .args([
            "tree",
            "--package",
            crate_name,
            "--edges",
            "normal,build",
            "--prefix",
            "none",
            "--all-features",
            "--quiet",
        ])
        .output()
        .map_err(|e| format!("running `cargo tree` for {crate_name}: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "`cargo tree` failed for {crate_name}:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut packages: Vec<String> = text
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_owned)
        .collect();
    packages.sort();
    packages.dedup();
    Ok(packages)
}
