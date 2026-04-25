//! Command handler modules for the CLI.

mod add;
mod cloud_sync;
mod cluster;
mod completions;
mod connect;
mod delete;
mod duplicate;
mod export_import;
mod group;
mod list;
mod manpage;
mod recording;
mod secret;
mod sftp;
mod show;
mod smart_folder;
mod snippet;
mod stats;
mod sync;
mod template;
mod test;
mod update;
mod variable;
mod wol;

use std::path::Path;

use crate::cli::Commands;
use crate::error::CliError;

/// Dispatch a CLI command to the appropriate handler.
#[allow(clippy::too_many_lines)]
pub fn dispatch(config_path: Option<&Path>, command: Commands) -> Result<(), CliError> {
    match command {
        Commands::List {
            format,
            protocol,
            group,
            tag,
        } => list::cmd_list(
            config_path,
            format.effective(),
            protocol.as_deref(),
            group.as_deref(),
            tag.as_deref(),
        ),
        Commands::Connect { name, dry_run } => connect::cmd_connect(config_path, &name, dry_run),
        Commands::Add {
            name,
            host,
            port,
            protocol,
            user,
            key,
            auth_method,
            device,
            baud_rate,
            icon,
            ssh_agent_socket,
            provider,
            hoop_connection_name,
            hoop_gateway_url,
            hoop_grpc_url,
            aws_profile,
            aws_region,
            gcp_zone,
            gcp_project,
            resource_group,
            bastion_name,
            vm_name,
            bastion_id,
            target_resource_id,
            target_private_ip,
            teleport_cluster,
            boundary_target,
            boundary_addr,
            custom_command,
            jump_host,
        } => add::cmd_add(
            config_path,
            add::AddParams {
                name: &name,
                host: &host,
                port,
                protocol: &protocol,
                user: user.as_deref(),
                key: key.as_deref(),
                auth_method: auth_method.as_deref(),
                device: device.as_deref(),
                baud_rate,
                icon: icon.as_deref(),
                ssh_agent_socket: ssh_agent_socket.as_deref(),
                provider: provider.as_deref(),
                hoop_connection_name: hoop_connection_name.as_deref(),
                hoop_gateway_url: hoop_gateway_url.as_deref(),
                hoop_grpc_url: hoop_grpc_url.as_deref(),
                aws_profile: aws_profile.as_deref(),
                aws_region: aws_region.as_deref(),
                gcp_zone: gcp_zone.as_deref(),
                gcp_project: gcp_project.as_deref(),
                resource_group: resource_group.as_deref(),
                bastion_name: bastion_name.as_deref(),
                vm_name: vm_name.as_deref(),
                bastion_id: bastion_id.as_deref(),
                target_resource_id: target_resource_id.as_deref(),
                target_private_ip: target_private_ip.as_deref(),
                teleport_cluster: teleport_cluster.as_deref(),
                boundary_target: boundary_target.as_deref(),
                boundary_addr: boundary_addr.as_deref(),
                custom_command: custom_command.as_deref(),
                jump_host: jump_host.as_deref(),
            },
        ),
        Commands::Export { format, output } => {
            export_import::cmd_export(config_path, format, &output)
        }
        Commands::Import { format, file } => export_import::cmd_import(config_path, format, &file),
        Commands::Test { name, timeout } => test::cmd_test(config_path, &name, timeout),
        Commands::Delete { name, force } => delete::cmd_delete(config_path, &name, force),
        Commands::Show { name } => show::cmd_show(config_path, &name),
        Commands::Update {
            name,
            new_name,
            host,
            port,
            user,
            key,
            auth_method,
            device,
            baud_rate,
            icon,
            ssh_agent_socket,
            provider,
            hoop_connection_name,
            hoop_gateway_url,
            hoop_grpc_url,
            aws_profile,
            aws_region,
            gcp_zone,
            gcp_project,
            resource_group,
            bastion_name,
            vm_name,
            bastion_id,
            target_resource_id,
            target_private_ip,
            teleport_cluster,
            boundary_target,
            boundary_addr,
            custom_command,
            jump_host,
        } => update::cmd_update(
            config_path,
            update::UpdateParams {
                name: &name,
                new_name: new_name.as_deref(),
                host: host.as_deref(),
                port,
                user: user.as_deref(),
                key: key.as_deref(),
                auth_method: auth_method.as_deref(),
                device: device.as_deref(),
                baud_rate,
                icon: icon.as_deref(),
                ssh_agent_socket: ssh_agent_socket.as_deref(),
                provider: provider.as_deref(),
                hoop_connection_name: hoop_connection_name.as_deref(),
                hoop_gateway_url: hoop_gateway_url.as_deref(),
                hoop_grpc_url: hoop_grpc_url.as_deref(),
                aws_profile: aws_profile.as_deref(),
                aws_region: aws_region.as_deref(),
                gcp_zone: gcp_zone.as_deref(),
                gcp_project: gcp_project.as_deref(),
                resource_group: resource_group.as_deref(),
                bastion_name: bastion_name.as_deref(),
                vm_name: vm_name.as_deref(),
                bastion_id: bastion_id.as_deref(),
                target_resource_id: target_resource_id.as_deref(),
                target_private_ip: target_private_ip.as_deref(),
                teleport_cluster: teleport_cluster.as_deref(),
                boundary_target: boundary_target.as_deref(),
                boundary_addr: boundary_addr.as_deref(),
                custom_command: custom_command.as_deref(),
                jump_host: jump_host.as_deref(),
            },
        ),
        Commands::Wol {
            target,
            broadcast,
            port,
        } => wol::cmd_wol(config_path, &target, &broadcast, port),
        Commands::Snippet(subcmd) => snippet::cmd_snippet(config_path, subcmd),
        Commands::Group(subcmd) => group::cmd_group(config_path, subcmd),
        Commands::Template(subcmd) => template::cmd_template(config_path, subcmd),
        Commands::Cluster(subcmd) => cluster::cmd_cluster(config_path, subcmd),
        Commands::Var(subcmd) => variable::cmd_var(config_path, subcmd),
        Commands::Secret(subcmd) => secret::cmd_secret(config_path, subcmd),
        Commands::SmartFolder(subcmd) => smart_folder::cmd_smart_folder(config_path, subcmd),
        Commands::Recording(subcmd) => recording::cmd_recording(subcmd),
        Commands::Duplicate { name, new_name } => {
            duplicate::cmd_duplicate(config_path, &name, new_name.as_deref())
        }
        Commands::Sftp { name, cli, mc } => sftp::cmd_sftp(config_path, &name, cli, mc),
        Commands::Stats => stats::cmd_stats(config_path),
        Commands::Completions { shell } => completions::cmd_completions(shell),
        Commands::ManPage => manpage::cmd_manpage(),
        Commands::Sync(subcmd) => cloud_sync::cmd_cloud_sync(config_path, subcmd),
    }
}
