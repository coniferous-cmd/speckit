/// Apply change workflow template.

pub const APPLY_CHANGE_TEMPLATE: &str = r#"# Apply Change

Read context files, work through pending tasks, mark complete as you go.
Pause if you hit blockers or need clarification.

## Steps

1. Read the change's proposal.md, specs/, design.md, and tasks.md
2. Identify pending tasks (unchecked checkboxes)
3. Work through tasks in order
4. Mark tasks complete as you finish them
5. Stop if you hit a blocker or need clarification
"#;
