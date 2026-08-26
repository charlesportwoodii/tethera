use tethera_common::structs::agent::{Agent, AgentSpawn};
use tethera_common::traits::AgentTrait;

#[test]
fn claude_launch_command_is_bare_when_no_first_prompt_is_given() {
    let spawn = AgentSpawn::new(Agent::Claude, "/home/charl/projects".to_string(), None);

    assert_eq!(Agent::Claude.launch_command(&spawn), vec!["claude".to_string()]);
}

#[test]
fn claude_launch_command_appends_the_first_prompt_as_one_argument() {
    let spawn = AgentSpawn::new(
        Agent::Claude,
        "/home/charl/projects".to_string(),
        Some("investigate the flaky test".to_string()),
    );

    assert_eq!(
        Agent::Claude.launch_command(&spawn),
        vec![
            "claude".to_string(),
            "investigate the flaky test".to_string()
        ]
    );
}

#[test]
fn resume_command_carries_the_session_id() {
    assert_eq!(
        Agent::Claude.resume_command("abc123"),
        vec![
            "claude".to_string(),
            "--resume".to_string(),
            "abc123".to_string()
        ]
    );
}

#[test]
fn each_agent_dispatches_to_its_own_binary() {
    let spawn = AgentSpawn::new(Agent::Codex, "/tmp".to_string(), None);

    assert_eq!(Agent::Codex.launch_command(&spawn), vec!["codex".to_string()]);
}
