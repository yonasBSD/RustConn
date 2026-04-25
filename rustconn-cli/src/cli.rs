//! CLI argument parsing types using `clap`.

use std::io::IsTerminal;
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

use crate::util::parse_key_val;

/// `RustConn` command-line interface for managing remote connections
#[derive(Parser)]
#[command(name = "rustconn-cli")]
#[command(author, version, about = "RustConn command-line interface")]
#[command(propagate_version = true)]
pub struct Cli {
    /// Path to the configuration directory
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,

    /// Increase output verbosity (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Suppress all output except errors
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Disable colored output
    #[arg(long, global = true, env = "NO_COLOR")]
    pub no_color: bool,

    #[command(subcommand)]
    pub command: Commands,
}

/// Available CLI commands
#[derive(Subcommand)]
pub enum Commands {
    /// List all connections
    #[command(about = "List all connections in the configuration")]
    List {
        /// Output format for the connection list
        #[arg(short, long, default_value = "table", value_enum)]
        format: OutputFormat,

        /// Filter connections by protocol (ssh, rdp, vnc, spice)
        #[arg(short, long)]
        protocol: Option<String>,

        /// Filter connections by group name
        #[arg(short, long)]
        group: Option<String>,

        /// Filter connections by tag
        #[arg(short, long)]
        tag: Option<String>,
    },

    /// Connect to a server by name or ID
    #[command(about = "Initiate a connection to a remote server")]
    Connect {
        /// Connection name or UUID
        name: String,

        /// Show the command that would be executed without running it
        #[arg(long)]
        dry_run: bool,
    },

    /// Add a new connection
    #[command(about = "Add a new connection to the configuration")]
    Add {
        /// Name for the new connection
        #[arg(short, long)]
        name: String,

        /// Host address (hostname or IP), or device path for serial
        #[arg(short = 'H', long)]
        host: String,

        /// Port number (defaults to protocol default: SSH=22, RDP=3389,
        /// VNC=5900)
        #[arg(short, long)]
        port: Option<u16>,

        /// Protocol type (ssh, rdp, vnc, spice, sftp, telnet, serial,
        /// mosh, kubernetes/k8s, zerotrust/zt)
        #[arg(short = 'P', long, default_value = "ssh")]
        protocol: String,

        /// Username for authentication
        #[arg(short, long)]
        user: Option<String>,

        /// Path to SSH private key file
        #[arg(short, long)]
        key: Option<PathBuf>,

        /// SSH authentication method (password, publickey,
        /// keyboard-interactive, agent, security-key)
        #[arg(long, value_name = "METHOD")]
        auth_method: Option<String>,

        /// Serial device path (e.g., /dev/ttyUSB0). Alias for --host
        /// with serial protocol
        #[arg(long, value_name = "PATH")]
        device: Option<String>,

        /// Serial baud rate (default: 115200)
        #[arg(long, default_value = "115200")]
        baud_rate: Option<u32>,

        /// Custom icon (emoji/unicode or GTK icon name, e.g. "🏢",
        /// "starred-symbolic")
        #[arg(long)]
        icon: Option<String>,

        /// Custom SSH agent socket path (overrides global and auto-detected socket)
        #[arg(long, value_name = "PATH")]
        ssh_agent_socket: Option<String>,

        /// Zero Trust provider (for zerotrust/zt protocol)
        #[arg(
            long,
            value_name = "PROVIDER",
            value_parser = ["aws_ssm", "gcp_iap", "azure_bastion", "azure_ssh", "cloudflare_access", "teleport", "tailscale_ssh", "oci_bastion", "boundary", "hoop_dev", "generic"]
        )]
        provider: Option<String>,

        /// Hoop.dev connection name (required for --provider hoop_dev)
        #[arg(long, value_name = "NAME")]
        hoop_connection_name: Option<String>,

        /// Hoop.dev gateway URL (optional, for --provider hoop_dev)
        #[arg(long, value_name = "URL")]
        hoop_gateway_url: Option<String>,

        /// Hoop.dev gRPC URL (optional, for --provider hoop_dev)
        #[arg(long, value_name = "URL")]
        hoop_grpc_url: Option<String>,

        /// AWS SSM instance ID (for --provider aws_ssm, uses --host as target if not set)
        #[arg(long, value_name = "PROFILE")]
        aws_profile: Option<String>,

        /// AWS region (for --provider aws_ssm)
        #[arg(long, value_name = "REGION")]
        aws_region: Option<String>,

        /// GCP zone (for --provider gcp_iap)
        #[arg(long, value_name = "ZONE")]
        gcp_zone: Option<String>,

        /// GCP project (for --provider gcp_iap)
        #[arg(long, value_name = "PROJECT")]
        gcp_project: Option<String>,

        /// Azure resource group (for --provider azure_bastion or azure_ssh)
        #[arg(long, value_name = "GROUP")]
        resource_group: Option<String>,

        /// Azure Bastion host name (for --provider azure_bastion)
        #[arg(long, value_name = "NAME")]
        bastion_name: Option<String>,

        /// Azure VM name (for --provider azure_ssh)
        #[arg(long, value_name = "NAME")]
        vm_name: Option<String>,

        /// OCI Bastion OCID (for --provider oci_bastion)
        #[arg(long, value_name = "OCID")]
        bastion_id: Option<String>,

        /// OCI target resource OCID (for --provider oci_bastion)
        #[arg(long, value_name = "OCID")]
        target_resource_id: Option<String>,

        /// OCI target private IP (for --provider oci_bastion)
        #[arg(long, value_name = "IP")]
        target_private_ip: Option<String>,

        /// Teleport cluster (for --provider teleport)
        #[arg(long, value_name = "CLUSTER")]
        teleport_cluster: Option<String>,

        /// Boundary target (for --provider boundary)
        #[arg(long, value_name = "TARGET")]
        boundary_target: Option<String>,

        /// Boundary address (for --provider boundary)
        #[arg(long, value_name = "URL")]
        boundary_addr: Option<String>,

        /// Generic command template (for --provider generic)
        /// Placeholders: {host}, {user}, {port}
        #[arg(long, value_name = "COMMAND")]
        custom_command: Option<String>,

        /// Existing SSH connection name or UUID to use as a jump host
        /// (sets jump_host_id for SSH/RDP/VNC/SPICE/SFTP connections)
        #[arg(long, value_name = "NAME|UUID")]
        jump_host: Option<String>,
    },

    /// Export connections to external format
    #[command(about = "Export connections to various formats")]
    Export {
        /// Export format
        #[arg(short, long, value_enum)]
        format: ExportFormatArg,

        /// Output file or directory path
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Import connections from external format
    #[command(about = "Import connections from various formats")]
    Import {
        /// Import format
        #[arg(short, long, value_enum)]
        format: ImportFormatArg,

        /// Input file path
        file: PathBuf,
    },

    /// Test connection connectivity
    #[command(about = "Test connectivity to a connection")]
    Test {
        /// Connection name or ID (use "all" to test all connections)
        name: String,

        /// Connection timeout in seconds
        #[arg(short, long, default_value = "10")]
        timeout: u64,
    },

    /// Delete a connection
    #[command(about = "Delete a connection")]
    Delete {
        /// Connection name or UUID
        name: String,

        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },

    /// Show connection details
    #[command(about = "Show connection details")]
    Show {
        /// Connection name or UUID
        name: String,
    },

    /// Update a connection
    #[command(about = "Update an existing connection")]
    Update {
        /// Connection name or UUID
        name: String,

        /// New name
        #[arg(short, long)]
        new_name: Option<String>,

        /// New host
        #[arg(short = 'H', long)]
        host: Option<String>,

        /// New port
        #[arg(short, long)]
        port: Option<u16>,

        /// New username
        #[arg(short, long)]
        user: Option<String>,

        /// Path to SSH private key file
        #[arg(short, long)]
        key: Option<PathBuf>,

        /// SSH authentication method (password, publickey,
        /// keyboard-interactive, agent, security-key)
        #[arg(long, value_name = "METHOD")]
        auth_method: Option<String>,

        /// Serial device path
        #[arg(long, value_name = "PATH")]
        device: Option<String>,

        /// Serial baud rate
        #[arg(long)]
        baud_rate: Option<u32>,

        /// Custom icon (emoji/unicode or GTK icon name, e.g. "🏢",
        /// "starred-symbolic")
        #[arg(long)]
        icon: Option<String>,

        /// Custom SSH agent socket path (overrides global and auto-detected socket)
        #[arg(long, value_name = "PATH")]
        ssh_agent_socket: Option<String>,

        /// Zero Trust provider (for zerotrust/zt protocol)
        #[arg(
            long,
            value_name = "PROVIDER",
            value_parser = ["aws_ssm", "gcp_iap", "azure_bastion", "azure_ssh", "cloudflare_access", "teleport", "tailscale_ssh", "oci_bastion", "boundary", "hoop_dev", "generic"]
        )]
        provider: Option<String>,

        /// Hoop.dev connection name (for --provider hoop_dev)
        #[arg(long, value_name = "NAME")]
        hoop_connection_name: Option<String>,

        /// Hoop.dev gateway URL (optional, for --provider hoop_dev)
        #[arg(long, value_name = "URL")]
        hoop_gateway_url: Option<String>,

        /// Hoop.dev gRPC URL (optional, for --provider hoop_dev)
        #[arg(long, value_name = "URL")]
        hoop_grpc_url: Option<String>,

        /// AWS profile (for --provider aws_ssm)
        #[arg(long, value_name = "PROFILE")]
        aws_profile: Option<String>,

        /// AWS region (for --provider aws_ssm)
        #[arg(long, value_name = "REGION")]
        aws_region: Option<String>,

        /// GCP zone (for --provider gcp_iap)
        #[arg(long, value_name = "ZONE")]
        gcp_zone: Option<String>,

        /// GCP project (for --provider gcp_iap)
        #[arg(long, value_name = "PROJECT")]
        gcp_project: Option<String>,

        /// Azure resource group (for --provider azure_bastion or azure_ssh)
        #[arg(long, value_name = "GROUP")]
        resource_group: Option<String>,

        /// Azure Bastion host name (for --provider azure_bastion)
        #[arg(long, value_name = "NAME")]
        bastion_name: Option<String>,

        /// Azure VM name (for --provider azure_ssh)
        #[arg(long, value_name = "NAME")]
        vm_name: Option<String>,

        /// OCI Bastion OCID (for --provider oci_bastion)
        #[arg(long, value_name = "OCID")]
        bastion_id: Option<String>,

        /// OCI target resource OCID (for --provider oci_bastion)
        #[arg(long, value_name = "OCID")]
        target_resource_id: Option<String>,

        /// OCI target private IP (for --provider oci_bastion)
        #[arg(long, value_name = "IP")]
        target_private_ip: Option<String>,

        /// Teleport cluster (for --provider teleport)
        #[arg(long, value_name = "CLUSTER")]
        teleport_cluster: Option<String>,

        /// Boundary target (for --provider boundary)
        #[arg(long, value_name = "TARGET")]
        boundary_target: Option<String>,

        /// Boundary address (for --provider boundary)
        #[arg(long, value_name = "URL")]
        boundary_addr: Option<String>,

        /// Generic command template (for --provider generic)
        #[arg(long, value_name = "COMMAND")]
        custom_command: Option<String>,

        /// Existing SSH connection name or UUID to use as a jump host
        /// (sets jump_host_id for SSH/RDP/VNC/SPICE/SFTP connections)
        #[arg(long, value_name = "NAME|UUID")]
        jump_host: Option<String>,
    },

    /// Send Wake-on-LAN magic packet
    #[command(about = "Wake a sleeping machine using Wake-on-LAN")]
    Wol {
        /// Connection name or MAC address
        /// (format: AA:BB:CC:DD:EE:FF or AA-BB-CC-DD-EE-FF)
        target: String,

        /// Broadcast address (default: 255.255.255.255)
        #[arg(short, long, default_value = "255.255.255.255")]
        broadcast: String,

        /// UDP port (default: 9)
        #[arg(short, long, default_value = "9")]
        port: u16,
    },

    /// Manage command snippets
    #[command(subcommand, about = "Manage command snippets")]
    Snippet(SnippetCommands),

    /// Manage connection groups
    #[command(subcommand, about = "Manage connection groups")]
    Group(GroupCommands),

    /// Manage connection templates
    #[command(subcommand, about = "Manage connection templates")]
    Template(TemplateCommands),

    /// Manage connection clusters
    #[command(subcommand, about = "Manage connection clusters")]
    Cluster(ClusterCommands),

    /// Manage global variables
    #[command(subcommand, about = "Manage global variables")]
    Var(VariableCommands),

    /// Manage secret backends and credentials
    #[command(subcommand, about = "Manage secret backends and credentials")]
    Secret(SecretCommands),

    /// Manage smart folders
    #[command(
        subcommand,
        about = "Manage smart folders for dynamic connection grouping"
    )]
    SmartFolder(SmartFolderCommands),

    /// Manage session recordings
    #[command(subcommand, about = "Manage session recordings")]
    Recording(RecordingCommands),

    /// Duplicate a connection
    #[command(about = "Duplicate an existing connection")]
    Duplicate {
        /// Connection name or UUID to duplicate
        name: String,

        /// New name for the duplicated connection
        #[arg(short, long)]
        new_name: Option<String>,
    },

    /// Open SFTP session for an SSH connection
    #[command(about = "Open SFTP file browser or CLI session for an SSH connection")]
    Sftp {
        /// Connection name or UUID
        name: String,

        /// Use sftp CLI instead of file manager
        #[arg(long)]
        cli: bool,

        /// Open SFTP via Midnight Commander (mc) in terminal
        #[arg(long)]
        mc: bool,
    },

    /// Show connection statistics
    #[command(about = "Show connection statistics")]
    Stats,

    /// Generate shell completions
    #[command(about = "Generate shell completion scripts")]
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },

    /// Generate man page
    #[command(about = "Generate man page and write to stdout")]
    ManPage,

    /// Cloud Sync and inventory sync operations
    #[command(subcommand, about = "Cloud Sync operations and inventory sync")]
    Sync(SyncCommands),
}

/// Output format for the list command
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum OutputFormat {
    /// Display as formatted table
    Table,
    /// Output as JSON
    Json,
    /// Output as CSV
    Csv,
}

impl OutputFormat {
    /// Returns the effective format, defaulting to JSON when stdout is not a terminal.
    ///
    /// Per clig.dev: "If stdin or stdout is not an interactive terminal,
    /// prefer structured output."
    #[must_use]
    pub fn effective(self) -> Self {
        if matches!(self, Self::Table) && !std::io::stdout().is_terminal() {
            Self::Json
        } else {
            self
        }
    }
}

/// Export format options
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ExportFormatArg {
    /// Ansible inventory format (INI or YAML)
    Ansible,
    /// OpenSSH config format
    SshConfig,
    /// Remmina connection files
    Remmina,
    /// Asbru-CM YAML format
    Asbru,
    /// Native `RustConn` format (.rcn)
    Native,
    /// Royal TS XML format (.rtsz)
    RoyalTs,
    /// MobaXterm session format (.mxtsessions)
    MobaXterm,
    /// CSV format (.csv)
    Csv,
}

/// Import format options
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ImportFormatArg {
    /// Ansible inventory format
    Ansible,
    /// OpenSSH config format
    SshConfig,
    /// Remmina connection files
    Remmina,
    /// Asbru-CM YAML format
    Asbru,
    /// Native `RustConn` format (.rcn)
    Native,
    /// Royal TS XML format (.rtsz)
    RoyalTs,
    /// MobaXterm session format (.mxtsessions)
    MobaXterm,
    /// Microsoft RDP file (.rdp)
    #[value(name = "rdp", alias = "rdp-file")]
    Rdp,
    /// Remote Desktop Manager JSON export
    #[value(name = "rdm", alias = "remote-desktop-manager")]
    Rdm,
    /// Virt-Viewer connection file (.vv)
    #[value(name = "virt-viewer", alias = "vv")]
    VirtViewer,
    /// Libvirt domain XML / GNOME Boxes
    #[value(name = "libvirt", alias = "gnome-boxes")]
    Libvirt,
    /// CSV format (.csv)
    Csv,
}

/// Snippet subcommands
#[derive(Subcommand)]
pub enum SnippetCommands {
    /// List all snippets
    #[command(about = "List all command snippets")]
    List {
        /// Output format
        #[arg(short, long, default_value = "table", value_enum)]
        format: OutputFormat,

        /// Filter by category
        #[arg(short, long)]
        category: Option<String>,

        /// Filter by tag
        #[arg(short, long)]
        tag: Option<String>,
    },

    /// Show snippet details
    #[command(about = "Show snippet details and variables")]
    Show {
        /// Snippet name or ID
        name: String,
    },

    /// Add a new snippet
    #[command(about = "Add a new command snippet")]
    Add {
        /// Snippet name
        #[arg(short, long)]
        name: String,

        /// Command template (use ${var} for variables)
        #[arg(short, long)]
        command: String,

        /// Description
        #[arg(short, long)]
        description: Option<String>,

        /// Category
        #[arg(long)]
        category: Option<String>,

        /// Tags (comma-separated)
        #[arg(short, long)]
        tags: Option<String>,
    },

    /// Delete a snippet
    #[command(about = "Delete a command snippet")]
    Delete {
        /// Snippet name or ID
        name: String,
    },

    /// Execute a snippet with variable substitution
    #[command(about = "Show snippet command with variable substitution")]
    Run {
        /// Snippet name or ID
        name: String,

        /// Variable values (format: var=value, can be repeated)
        #[arg(short, long, value_parser = parse_key_val)]
        var: Vec<(String, String)>,

        /// Actually execute the command (default: just print)
        #[arg(short, long)]
        execute: bool,
    },
}

/// Group subcommands
#[derive(Subcommand)]
pub enum GroupCommands {
    /// List all groups
    #[command(about = "List all connection groups")]
    List {
        /// Output format
        #[arg(short, long, default_value = "table", value_enum)]
        format: OutputFormat,
    },

    /// Show group details
    #[command(about = "Show group details and connections")]
    Show {
        /// Group name or ID
        name: String,
    },

    /// Create a new group
    #[command(about = "Create a new connection group")]
    Create {
        /// Group name
        #[arg(short, long)]
        name: String,

        /// Parent group name or ID
        #[arg(short, long)]
        parent: Option<String>,

        /// Description
        #[arg(short, long)]
        description: Option<String>,

        /// Custom icon (emoji/unicode or GTK icon name, e.g. "🏢",
        /// "starred-symbolic")
        #[arg(long)]
        icon: Option<String>,
    },

    /// Delete a group
    #[command(about = "Delete a connection group")]
    Delete {
        /// Group name or ID
        name: String,
    },

    /// Add a connection to a group
    #[command(about = "Add a connection to a group")]
    AddConnection {
        /// Group name or ID
        #[arg(short, long)]
        group: String,

        /// Connection name or ID
        #[arg(short, long)]
        connection: String,
    },

    /// Remove a connection from a group
    #[command(about = "Remove a connection from a group")]
    RemoveConnection {
        /// Group name or ID
        #[arg(short, long)]
        group: String,

        /// Connection name or ID
        #[arg(short, long)]
        connection: String,
    },

    /// Edit group SSH inheritance settings
    #[command(about = "Edit group properties (SSH inheritance fields)")]
    Edit {
        /// Group name or ID
        name: String,

        /// SSH key path for inheritance by child connections (local-only)
        #[arg(long)]
        ssh_key_path: Option<String>,

        /// SSH authentication method (password, publickey, agent,
        /// keyboard-interactive, security-key)
        #[arg(long)]
        ssh_auth_method: Option<String>,

        /// SSH ProxyJump host for inheritance
        #[arg(long)]
        ssh_proxy_jump: Option<String>,

        /// SSH agent socket override for inheritance (local-only)
        #[arg(long)]
        ssh_agent_socket: Option<String>,
    },
}

/// Template subcommands
#[derive(Subcommand)]
pub enum TemplateCommands {
    /// List all templates
    #[command(about = "List all connection templates")]
    List {
        /// Output format
        #[arg(short, long, default_value = "table", value_enum)]
        format: OutputFormat,

        /// Filter by protocol (ssh, rdp, vnc, spice)
        #[arg(short, long)]
        protocol: Option<String>,
    },

    /// Show template details
    #[command(about = "Show template details")]
    Show {
        /// Template name or ID
        name: String,
    },

    /// Create a new template
    #[command(about = "Create a new connection template")]
    Create {
        /// Template name
        #[arg(short, long)]
        name: String,

        /// Protocol type (ssh, rdp, vnc, spice)
        #[arg(short = 'P', long, default_value = "ssh")]
        protocol: String,

        /// Default host
        #[arg(short = 'H', long)]
        host: Option<String>,

        /// Default port
        #[arg(short, long)]
        port: Option<u16>,

        /// Default username
        #[arg(short, long)]
        user: Option<String>,

        /// Description
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Delete a template
    #[command(about = "Delete a connection template")]
    Delete {
        /// Template name or ID
        name: String,
    },

    /// Create a connection from a template
    #[command(about = "Create a new connection from a template")]
    Apply {
        /// Template name or ID
        template: String,

        /// Name for the new connection
        #[arg(short, long)]
        name: Option<String>,

        /// Override host
        #[arg(short = 'H', long)]
        host: Option<String>,

        /// Override port
        #[arg(short, long)]
        port: Option<u16>,

        /// Override username
        #[arg(short, long)]
        user: Option<String>,
    },
}

/// Cluster subcommands
#[derive(Subcommand)]
pub enum ClusterCommands {
    /// List all clusters
    #[command(about = "List all connection clusters")]
    List {
        /// Output format
        #[arg(short, long, default_value = "table", value_enum)]
        format: OutputFormat,
    },

    /// Show cluster details
    #[command(about = "Show cluster details and connections")]
    Show {
        /// Cluster name or ID
        name: String,
    },

    /// Create a new cluster
    #[command(about = "Create a new connection cluster")]
    Create {
        /// Cluster name
        #[arg(short, long)]
        name: String,

        /// Connection names or IDs to include (comma-separated)
        #[arg(short, long)]
        connections: Option<String>,

        /// Enable broadcast mode by default
        #[arg(short, long)]
        broadcast: bool,
    },

    /// Delete a cluster
    #[command(about = "Delete a connection cluster")]
    Delete {
        /// Cluster name or ID
        name: String,
    },

    /// Add a connection to a cluster
    #[command(about = "Add a connection to a cluster")]
    AddConnection {
        /// Cluster name or ID
        #[arg(short = 'C', long)]
        cluster: String,

        /// Connection name or ID
        #[arg(short, long)]
        connection: String,
    },

    /// Remove a connection from a cluster
    #[command(about = "Remove a connection from a cluster")]
    RemoveConnection {
        /// Cluster name or ID
        #[arg(short = 'C', long)]
        cluster: String,

        /// Connection name or ID
        #[arg(short, long)]
        connection: String,
    },
}

/// Variable subcommands
#[derive(Subcommand)]
pub enum VariableCommands {
    /// List all global variables
    #[command(about = "List all global variables")]
    List {
        /// Output format
        #[arg(short, long, default_value = "table", value_enum)]
        format: OutputFormat,
    },

    /// Show variable details
    #[command(about = "Show variable value")]
    Show {
        /// Variable name
        name: String,
    },

    /// Set a global variable
    #[command(about = "Set a global variable value")]
    Set {
        /// Variable name
        name: String,

        /// Variable value
        value: String,

        /// Mark as secret (value will be masked in output)
        #[arg(short, long)]
        secret: bool,

        /// Description
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Delete a global variable
    #[command(about = "Delete a global variable")]
    Delete {
        /// Variable name
        name: String,
    },
}

/// Secret backend subcommands
#[derive(Subcommand)]
pub enum SecretCommands {
    /// Show available secret backends and their status
    #[command(about = "Show available secret backends and their status")]
    Status,

    /// Get password for a connection from secret backend
    #[command(about = "Get password for a connection from secret backend")]
    Get {
        /// Connection name or ID
        connection: String,

        /// Secret backend to use
        /// (keyring, keepass, bitwarden, 1password, passbolt)
        #[arg(short, long)]
        backend: Option<String>,
    },

    /// Store password for a connection in secret backend
    #[command(about = "Store password for a connection in secret backend")]
    Set {
        /// Connection name or ID
        connection: String,

        /// Username (optional, uses connection username if not specified)
        #[arg(short, long)]
        user: Option<String>,

        /// Password (if not provided, will prompt interactively)
        #[arg(short, long)]
        password: Option<String>,

        /// Secret backend to use
        /// (keyring, keepass, bitwarden, 1password, passbolt)
        #[arg(short, long)]
        backend: Option<String>,
    },

    /// Delete password for a connection from secret backend
    #[command(about = "Delete password for a connection from secret backend")]
    Delete {
        /// Connection name or ID
        connection: String,

        /// Secret backend to use
        /// (keyring, keepass, bitwarden, 1password, passbolt)
        #[arg(short, long)]
        backend: Option<String>,
    },

    /// Verify KeePass database credentials
    #[command(about = "Verify KeePass database credentials")]
    VerifyKeepass {
        /// Path to KDBX file
        #[arg(short, long)]
        database: PathBuf,

        /// Path to key file (optional)
        #[arg(short, long)]
        key_file: Option<PathBuf>,
    },
}

/// Smart folder subcommands
#[derive(Subcommand)]
pub enum SmartFolderCommands {
    /// List all smart folders
    #[command(about = "List all smart folders")]
    List {
        /// Output format
        #[arg(short, long, default_value = "table", value_enum)]
        format: OutputFormat,
    },

    /// Show connections matching a smart folder
    #[command(about = "Show connections matching a smart folder's filters")]
    Show {
        /// Smart folder name or ID
        name: String,
    },

    /// Create a new smart folder
    #[command(about = "Create a new smart folder with filter criteria")]
    Create {
        /// Smart folder name
        #[arg(short, long)]
        name: String,

        /// Filter by protocol (ssh, rdp, vnc, etc.)
        #[arg(short, long)]
        protocol: Option<String>,

        /// Filter by host glob pattern (e.g. "*.prod.*")
        #[arg(short = 'H', long)]
        host_pattern: Option<String>,

        /// Filter by tags (comma-separated)
        #[arg(short, long)]
        tags: Option<String>,
    },

    /// Delete a smart folder
    #[command(about = "Delete a smart folder")]
    Delete {
        /// Smart folder name or ID
        name: String,
    },
}

/// Recording subcommands
#[derive(Subcommand)]
pub enum RecordingCommands {
    /// List all recordings with metadata
    #[command(about = "List all recordings with metadata")]
    List {
        /// Output format
        #[arg(long, default_value = "table", value_enum)]
        format: OutputFormat,
    },

    /// Delete a recording by name
    #[command(about = "Delete a recording by name")]
    Delete {
        /// Recording display name or connection name
        name: String,

        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },

    /// Import external scriptreplay files
    #[command(about = "Import external scriptreplay files")]
    Import {
        /// Path to the data file
        data_file: PathBuf,

        /// Path to the timing file
        timing_file: PathBuf,
    },
}

/// Sync subcommands (Cloud Sync + inventory sync)
#[derive(Subcommand)]
pub enum SyncCommands {
    /// Show Cloud Sync status (sync directory, device name, per-group status)
    #[command(about = "Show Cloud Sync status")]
    Status,

    /// List all synced groups with mode and last sync time
    #[command(about = "List all synced groups")]
    List {
        /// Output format
        #[arg(short, long, default_value = "table", value_enum)]
        format: OutputFormat,
    },

    /// Export a Master group to its sync file
    #[command(about = "Export a Master group to its cloud sync file")]
    Export {
        /// Group name or ID to export
        group: String,
    },

    /// Import a .rcn sync file
    #[command(about = "Import a .rcn cloud sync file")]
    Import {
        /// Path to the .rcn file
        file: String,
    },

    /// Export all Master groups and import all Import groups
    #[command(about = "Sync now: export all Master groups, import all Import groups")]
    Now,

    /// Sync connections from a dynamic inventory source (JSON/YAML)
    #[command(about = "Sync connections from a dynamic inventory source (JSON/YAML)")]
    Inventory {
        /// Path to inventory file (JSON or YAML)
        file: PathBuf,

        /// Source identifier for tagging (e.g. "netbox", "ansible")
        #[arg(short, long)]
        source: String,

        /// Remove connections from this source that are no longer in the inventory
        #[arg(long)]
        remove_stale: bool,

        /// Dry run — show what would change without modifying anything
        #[arg(long)]
        dry_run: bool,
    },
}
