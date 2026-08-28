//! Show Command
//!
//! Show a change or spec (top-level verb-first command).

/// Options for the show command.
#[derive(Debug, Clone)]
pub struct ShowOptions {
    pub json: bool,
    pub item_type: Option<String>,
    pub no_interactive: bool,
    pub deltas_only: bool,
    pub requirements_only: bool,
    pub requirements: bool,
    pub no_scenarios: bool,
    pub requirement: Option<String>,
    pub store: Option<String>,
}

/// Execute the top-level show command.
pub async fn show_command(item_name: Option<&str>, options: ShowOptions) -> anyhow::Result<()> {
    let project_root = crate::change::resolve_project_root(options.store.as_deref()).await?;

    let type_override =
        options
            .item_type
            .as_deref()
            .and_then(|v| match v.to_lowercase().as_str() {
                "change" => Some("change"),
                "spec" => Some("spec"),
                _ => None,
            });

    if item_name.is_none() {
        if !options.no_interactive && atty_is_tty() {
            let choices = vec!["Change", "Spec"];
            let choice = inquire::Select::new("What would you like to show?", choices)
                .prompt()
                .map_err(|e| anyhow::anyhow!("Selection cancelled: {e}"))?;

            if choice == "Change" {
                let changes = crate::change::get_active_change_ids(&project_root).await?;
                if changes.is_empty() {
                    eprintln!("No changes found.");
                    std::process::exit(1);
                }
                let picked = inquire::Select::new("Pick a change", changes)
                    .prompt()
                    .map_err(|e| anyhow::anyhow!("Selection cancelled: {e}"))?;
                return crate::change::change_show(
                    Some(&picked),
                    crate::change::ChangeShowOptions {
                        json: options.json,
                        deltas_only: options.deltas_only,
                        requirements_only: options.requirements_only,
                        no_interactive: options.no_interactive,
                    },
                    Some(&project_root),
                )
                .await;
            }

            let specs = crate::spec::get_spec_ids(&project_root).await?;
            if specs.is_empty() {
                eprintln!("No specs found.");
                std::process::exit(1);
            }
            let picked = inquire::Select::new("Pick a spec", specs)
                .prompt()
                .map_err(|e| anyhow::anyhow!("Selection cancelled: {e}"))?;
            return crate::spec::spec_show(
                Some(&picked),
                crate::spec::SpecShowOptions {
                    json: options.json,
                    requirements: options.requirements,
                    no_scenarios: options.no_scenarios,
                    requirement: options.requirement.clone(),
                    no_interactive: options.no_interactive,
                },
                Some(&project_root),
            )
            .await;
        }

        eprintln!("Nothing to show. Try one of:");
        eprintln!("  speckit show <item>");
        eprintln!("  speckit change show");
        eprintln!("  speckit spec show");
        eprintln!("Or run in an interactive terminal.");
        std::process::exit(1);
    }

    let name = item_name.unwrap();

    // Resolve the item type
    let changes = crate::change::get_active_change_ids(&project_root).await?;
    let specs = crate::spec::get_spec_ids(&project_root).await?;

    let is_change = changes.contains(&name.to_string());
    let is_spec = specs.contains(&name.to_string());

    let resolved_type = type_override.or_else(|| {
        if is_change {
            Some("change")
        } else if is_spec {
            Some("spec")
        } else {
            None
        }
    });

    match resolved_type {
        Some("change") => {
            crate::change::change_show(
                Some(name),
                crate::change::ChangeShowOptions {
                    json: options.json,
                    deltas_only: options.deltas_only,
                    requirements_only: options.requirements_only,
                    no_interactive: options.no_interactive,
                },
                Some(&project_root),
            )
            .await
        }
        Some("spec") => {
            crate::spec::spec_show(
                Some(name),
                crate::spec::SpecShowOptions {
                    json: options.json,
                    requirements: options.requirements,
                    no_scenarios: options.no_scenarios,
                    requirement: options.requirement.clone(),
                    no_interactive: options.no_interactive,
                },
                Some(&project_root),
            )
            .await
        }
        _ => {
            let mut all_items = changes.clone();
            all_items.extend(specs.iter().cloned());
            let suggestions = nearest_matches(name, &all_items);
            let msg = if suggestions.is_empty() {
                format!("Unknown item '{name}'.")
            } else {
                format!(
                    "Unknown item '{name}'. Did you mean: {}?",
                    suggestions.join(", ")
                )
            };
            if options.json {
                crate::shared_output::print_json(&serde_json::json!({
                    "status": [{ "severity": "error", "code": "unknown_item", "message": msg }]
                }));
            } else {
                eprintln!("{msg}");
            }
            std::process::exit(1);
        }
    }
}

fn nearest_matches(query: &str, candidates: &[String]) -> Vec<String> {
    let query_lower = query.to_lowercase();
    let mut scored: Vec<(String, usize)> = candidates
        .iter()
        .map(|c| {
            let c_lower = c.to_lowercase();
            let distance = levenshtein_distance(&query_lower, &c_lower);
            (c.clone(), distance)
        })
        .collect();
    scored.sort_by_key(|(_, d)| *d);
    scored
        .into_iter()
        .take(5)
        .filter(|(_, d)| *d <= 5)
        .map(|(name, _)| name)
        .collect()
}

fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let n = a.len();
    let m = b.len();
    let mut dp = vec![vec![0; m + 1]; n + 1];
    for i in 0..=n {
        dp[i][0] = i;
    }
    for j in 0..=m {
        dp[0][j] = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }
    dp[n][m]
}

fn atty_is_tty() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stderr())
}
