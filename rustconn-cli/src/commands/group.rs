//! Group management commands.

use std::path::Path;
use std::path::PathBuf;

use rustconn_core::ConnectionManager;
use rustconn_core::models::ConnectionGroup;
use rustconn_core::models::SshAuthMethod;

use crate::cli::{GroupCommands, OutputFormat};
use crate::error::CliError;
use crate::format::escape_csv_field;
use crate::util::create_config_manager;

/// Group command handler
pub fn cmd_group(config_path: Option<&Path>, subcmd: GroupCommands) -> Result<(), CliError> {
    match subcmd {
        GroupCommands::List { format } => cmd_group_list(config_path, format.effective()),
        GroupCommands::Show { name } => cmd_group_show(config_path, &name),
        GroupCommands::Create {
            name,
            parent,
            description,
            icon,
        } => cmd_group_create(
            config_path,
            &name,
            parent.as_deref(),
            description.as_deref(),
            icon.as_deref(),
        ),
        GroupCommands::Delete { name } => cmd_group_delete(config_path, &name),
        GroupCommands::AddConnection { group, connection } => {
            cmd_group_add_connection(config_path, &group, &connection)
        }
        GroupCommands::RemoveConnection { group, connection } => {
            cmd_group_remove_connection(config_path, &group, &connection)
        }
        GroupCommands::Edit {
            name,
            ssh_key_path,
            ssh_auth_method,
            ssh_proxy_jump,
            ssh_agent_socket,
        } => cmd_group_edit(
            config_path,
            &name,
            ssh_key_path.as_deref(),
            ssh_auth_method.as_deref(),
            ssh_proxy_jump.as_deref(),
            ssh_agent_socket.as_deref(),
        ),
    }
}

fn cmd_group_list(config_path: Option<&Path>, format: OutputFormat) -> Result<(), CliError> {
    let config_manager = create_config_manager(config_path)?;

    let groups = config_manager
        .load_groups()
        .map_err(|e| CliError::Group(format!("Failed to load groups: {e}")))?;

    match format {
        OutputFormat::Table => print_group_table(&groups),
        OutputFormat::Json => print_group_json(&groups)?,
        OutputFormat::Csv => print_group_csv(&groups),
    }

    Ok(())
}

fn print_group_table(groups: &[ConnectionGroup]) {
    if groups.is_empty() {
        println!("No groups found.");
        return;
    }

    let name_width = groups
        .iter()
        .map(|g| g.name.len())
        .max()
        .unwrap_or(4)
        .max(4);

    println!("{:<name_width$}  PARENT", "NAME");
    println!("{:-<name_width$}  {:-<20}", "", "");

    for group in groups {
        let parent = group.parent_id.map_or_else(
            || "-".to_string(),
            |id| {
                groups
                    .iter()
                    .find(|g| g.id == id)
                    .map_or_else(|| id.to_string(), |g| g.name.clone())
            },
        );
        let parent_display = if parent.len() > 20 {
            format!("{}...", &parent[..17])
        } else {
            parent
        };
        println!("{:<name_width$}  {parent_display}", group.name);
    }
}

fn print_group_json(groups: &[ConnectionGroup]) -> Result<(), CliError> {
    let json = serde_json::to_string_pretty(groups)
        .map_err(|e| CliError::Group(format!("Failed to serialize: {e}")))?;
    println!("{json}");
    Ok(())
}

fn print_group_csv(groups: &[ConnectionGroup]) {
    println!("name,parent_id");
    for group in groups {
        let name = escape_csv_field(&group.name);
        let parent = group.parent_id.map(|id| id.to_string()).unwrap_or_default();
        println!("{name},{parent}");
    }
}

fn cmd_group_show(config_path: Option<&Path>, name: &str) -> Result<(), CliError> {
    let config_manager = create_config_manager(config_path)?;

    let groups = config_manager
        .load_groups()
        .map_err(|e| CliError::Group(format!("Failed to load groups: {e}")))?;

    let connections = config_manager
        .load_connections()
        .map_err(|e| CliError::Config(format!("Failed to load connections: {e}")))?;

    let group = find_group(&groups, name)?;

    println!("Group Details:");
    println!("  ID:   {}", group.id);
    println!("  Name: {}", group.name);

    if let Some(ref desc) = group.description {
        println!("  Description: {desc}");
    }
    if let Some(ref icon) = group.icon {
        println!("  Icon: {icon}");
    }

    if let Some(parent_id) = group.parent_id {
        let parent_name = groups
            .iter()
            .find(|g| g.id == parent_id)
            .map_or("(unknown)", |g| g.name.as_str());
        println!("  Parent: {parent_name} ({parent_id})");
    }

    // SSH inheritance fields
    if let Some(ref auth_method) = group.ssh_auth_method {
        let method = match auth_method {
            rustconn_core::models::SshAuthMethod::Password => "Password",
            rustconn_core::models::SshAuthMethod::PublicKey => "PublicKey",
            rustconn_core::models::SshAuthMethod::KeyboardInteractive => "KeyboardInteractive",
            rustconn_core::models::SshAuthMethod::Agent => "Agent",
            rustconn_core::models::SshAuthMethod::SecurityKey => "SecurityKey",
        };
        println!("  SSH Auth Method: {method}");
    }
    if let Some(ref key_path) = group.ssh_key_path {
        println!("  SSH Key Path: {}", key_path.display());
    }
    if let Some(ref proxy_jump) = group.ssh_proxy_jump {
        println!("  SSH Proxy Jump: {proxy_jump}");
    }
    if let Some(ref agent_socket) = group.ssh_agent_socket {
        println!("  SSH Agent Socket: {agent_socket}");
    }

    let group_connections: Vec<_> = connections
        .iter()
        .filter(|c| c.group_id == Some(group.id))
        .collect();

    println!("\nConnections ({}):", group_connections.len());
    for conn in &group_connections {
        println!("  - {} ({})", conn.name, conn.host);
    }

    Ok(())
}

fn cmd_group_create(
    config_path: Option<&Path>,
    name: &str,
    parent: Option<&str>,
    description: Option<&str>,
    icon: Option<&str>,
) -> Result<(), CliError> {
    let config_manager = create_config_manager(config_path)?;

    let mut groups = config_manager
        .load_groups()
        .map_err(|e| CliError::Group(format!("Failed to load groups: {e}")))?;

    if groups.iter().any(|g| g.name.eq_ignore_ascii_case(name)) {
        return Err(CliError::Group(format!(
            "Group with name '{name}' already exists"
        )));
    }

    let mut group = if let Some(parent_name) = parent {
        let parent_group = find_group(&groups, parent_name)?;
        ConnectionGroup::with_parent(name.to_string(), parent_group.id)
    } else {
        ConnectionGroup::new(name.to_string())
    };

    if let Some(desc) = description {
        group.description = Some(desc.to_string());
    }
    if let Some(ic) = icon {
        group.icon = Some(ic.to_string());
    }

    let id = group.id;
    groups.push(group);

    config_manager
        .save_groups(&groups)
        .map_err(|e| CliError::Group(format!("Failed to save groups: {e}")))?;

    println!("Created group '{name}' with ID {id}");

    Ok(())
}

fn cmd_group_delete(config_path: Option<&Path>, name: &str) -> Result<(), CliError> {
    let config_manager = create_config_manager(config_path)?;

    // Load groups to resolve the name/ID to a UUID
    let groups = config_manager
        .load_groups()
        .map_err(|e| CliError::Group(format!("Failed to load groups: {e}")))?;

    let group = find_group(&groups, name)?;
    let id = group.id;
    let group_name = group.name.clone();

    // ConnectionManager::new() spawns tokio tasks for debounced persistence,
    // so we need a runtime even though the CLI is synchronous.
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| CliError::Group(format!("Failed to create async runtime: {e}")))?;

    rt.block_on(async {
        let mut manager = ConnectionManager::new(config_manager).map_err(|e| {
            CliError::Group(format!("Failed to initialize connection manager: {e}"))
        })?;

        manager
            .delete_group(id)
            .map_err(|e| CliError::Group(format!("Failed to delete group: {e}")))?;

        manager
            .flush_persistence()
            .await
            .map_err(|e| CliError::Group(format!("Failed to persist changes: {e}")))?;

        Ok::<(), CliError>(())
    })?;

    println!("Deleted group '{group_name}' (ID: {id})");

    Ok(())
}

fn cmd_group_add_connection(
    config_path: Option<&Path>,
    group_name: &str,
    connection_name: &str,
) -> Result<(), CliError> {
    let config_manager = create_config_manager(config_path)?;

    let groups = config_manager
        .load_groups()
        .map_err(|e| CliError::Group(format!("Failed to load groups: {e}")))?;

    let mut connections = config_manager
        .load_connections()
        .map_err(|e| CliError::Config(format!("Failed to load connections: {e}")))?;

    let group = find_group(&groups, group_name)?;
    let group_id = group.id;
    let grp_name = group.name.clone();

    let connection = connections
        .iter_mut()
        .find(|c| {
            c.name.eq_ignore_ascii_case(connection_name) || c.id.to_string() == connection_name
        })
        .ok_or_else(|| CliError::ConnectionNotFound(connection_name.to_string()))?;

    if connection.group_id == Some(group_id) {
        return Err(CliError::Group(format!(
            "Connection '{}' is already in group '{grp_name}'",
            connection.name
        )));
    }

    let conn_name = connection.name.clone();
    connection.group_id = Some(group_id);

    config_manager
        .save_connections(&connections)
        .map_err(|e| CliError::Config(format!("Failed to save connections: {e}")))?;

    println!("Added connection '{conn_name}' to group '{grp_name}'");

    Ok(())
}

fn cmd_group_remove_connection(
    config_path: Option<&Path>,
    group_name: &str,
    connection_name: &str,
) -> Result<(), CliError> {
    let config_manager = create_config_manager(config_path)?;

    let groups = config_manager
        .load_groups()
        .map_err(|e| CliError::Group(format!("Failed to load groups: {e}")))?;

    let mut connections = config_manager
        .load_connections()
        .map_err(|e| CliError::Config(format!("Failed to load connections: {e}")))?;

    let group = find_group(&groups, group_name)?;
    let group_id = group.id;
    let grp_name = group.name.clone();

    let connection = connections
        .iter_mut()
        .find(|c| {
            c.name.eq_ignore_ascii_case(connection_name) || c.id.to_string() == connection_name
        })
        .ok_or_else(|| CliError::ConnectionNotFound(connection_name.to_string()))?;

    if connection.group_id != Some(group_id) {
        return Err(CliError::Group(format!(
            "Connection '{}' is not in group '{grp_name}'",
            connection.name
        )));
    }

    let conn_name = connection.name.clone();
    connection.group_id = None;

    config_manager
        .save_connections(&connections)
        .map_err(|e| CliError::Config(format!("Failed to save connections: {e}")))?;

    println!("Removed connection '{conn_name}' from group '{grp_name}'");

    Ok(())
}

fn cmd_group_edit(
    config_path: Option<&Path>,
    name: &str,
    ssh_key_path: Option<&str>,
    ssh_auth_method: Option<&str>,
    ssh_proxy_jump: Option<&str>,
    ssh_agent_socket: Option<&str>,
) -> Result<(), CliError> {
    if ssh_key_path.is_none()
        && ssh_auth_method.is_none()
        && ssh_proxy_jump.is_none()
        && ssh_agent_socket.is_none()
    {
        return Err(CliError::Group(
            "No fields to update. Use --ssh-key-path, --ssh-auth-method, \
             --ssh-proxy-jump, or --ssh-agent-socket"
                .to_string(),
        ));
    }

    let auth_method = ssh_auth_method.map(parse_ssh_auth_method).transpose()?;

    let config_manager = create_config_manager(config_path)?;

    let mut groups = config_manager
        .load_groups()
        .map_err(|e| CliError::Group(format!("Failed to load groups: {e}")))?;

    let group = groups
        .iter_mut()
        .find(|g| g.name.eq_ignore_ascii_case(name) || g.id.to_string() == name)
        .ok_or_else(|| CliError::Group(format!("Group not found: {name}")))?;

    let group_name = group.name.clone();
    let mut updated = Vec::new();

    if let Some(path) = ssh_key_path {
        group.ssh_key_path = Some(PathBuf::from(path));
        updated.push(format!("ssh_key_path = {path}"));
    }
    if let Some(method) = auth_method {
        group.ssh_auth_method = Some(method.clone());
        updated.push(format!("ssh_auth_method = {method:?}"));
    }
    if let Some(jump) = ssh_proxy_jump {
        group.ssh_proxy_jump = Some(jump.to_string());
        updated.push(format!("ssh_proxy_jump = {jump}"));
    }
    if let Some(socket) = ssh_agent_socket {
        group.ssh_agent_socket = Some(socket.to_string());
        updated.push(format!("ssh_agent_socket = {socket}"));
    }

    config_manager
        .save_groups(&groups)
        .map_err(|e| CliError::Group(format!("Failed to save groups: {e}")))?;

    println!("Updated group '{group_name}': {}", updated.join(", "));

    Ok(())
}

fn parse_ssh_auth_method(value: &str) -> Result<SshAuthMethod, CliError> {
    match value.to_lowercase().as_str() {
        "password" => Ok(SshAuthMethod::Password),
        "publickey" | "public_key" | "public-key" => Ok(SshAuthMethod::PublicKey),
        "agent" => Ok(SshAuthMethod::Agent),
        "keyboard-interactive" | "keyboard_interactive" | "keyboardinteractive" => {
            Ok(SshAuthMethod::KeyboardInteractive)
        }
        "security-key" | "security_key" | "securitykey" => Ok(SshAuthMethod::SecurityKey),
        _ => Err(CliError::Group(format!(
            "Invalid SSH auth method: '{value}'. \
             Valid values: password, publickey, agent, keyboard-interactive, security-key"
        ))),
    }
}

/// Find a group by name or ID
fn find_group<'a>(
    groups: &'a [ConnectionGroup],
    name_or_id: &str,
) -> Result<&'a ConnectionGroup, CliError> {
    if let Ok(uuid) = uuid::Uuid::parse_str(name_or_id)
        && let Some(group) = groups.iter().find(|g| g.id == uuid)
    {
        return Ok(group);
    }

    let matches: Vec<_> = groups
        .iter()
        .filter(|g| g.name.eq_ignore_ascii_case(name_or_id))
        .collect();

    match matches.len() {
        0 => Err(CliError::Group(format!("Group not found: {name_or_id}"))),
        1 => Ok(matches[0]),
        _ => Err(CliError::Group(format!(
            "Ambiguous group name: {name_or_id}"
        ))),
    }
}
