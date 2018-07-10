use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;

use clap::{Arg, SubCommand};
use toml;

use management::{ConsensusAction, LeaderAction, MgmtCommand};

use ConsensusState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub(crate) struct System {
    /// Logging level
    pub verbosity: String,

    /// Network settings
    pub network: Network,

    /// Consul settings
    pub consul: Consul,

    /// Metric settings
    pub metrics: Metrics,

    /// Carbon backend settings
    pub carbon: Carbon,

    /// Number of networking threads, use 0 for number of CPUs
    pub n_threads: usize,

    /// Number of aggregating(worker) threads, set to 0 to use all CPU cores
    pub w_threads: usize,

    /// queue size for single counting thread before packet is dropped
    pub task_queue_size: usize,

    /// Should we start as leader state enabled or not
    pub start_as_leader: bool,

    /// How often to gather own stats, in ms. Use 0 to disable (stats are still gathered, but not included in
    /// metric dump)
    pub stats_interval: u64,

    /// Prefix to send own metrics with
    pub stats_prefix: String,
}

impl Default for System {
    fn default() -> Self {
        Self {
            verbosity: "warn".to_string(),
            network: Network::default(),
            consul: Consul::default(),
            metrics: Metrics::default(),
            carbon: Carbon::default(),
            n_threads: 4,
            w_threads: 4,
            stats_interval: 10000,
            task_queue_size: 2048,
            start_as_leader: false,
            stats_prefix: "resources.monitoring.bioyino".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub(crate) struct Metrics {
    // TODO: Maximum metric array size, 0 for unlimited
    //  max_metrics: usize,
    /// Should we provide metrics with top update numbers
    pub count_updates: bool,

    /// Prefix for metric update statistics
    pub update_counter_prefix: String,

    /// Suffix for metric update statistics
    pub update_counter_suffix: String,

    /// Minimal update count to be reported
    pub update_counter_threshold: u32,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            //           max_metrics: 0,
            count_updates: true,
            update_counter_prefix: "resources.monitoring.bioyino.updates".to_string(),
            update_counter_suffix: String::new(),
            update_counter_threshold: 200,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub(crate) struct Carbon {
    // TODO: will be used when multiple backends support is implemented
    ///// Enable sending to carbon protocol backend
    //pub enabled: bool,
    /// IP and port of the carbon-protocol backend to send aggregated data to
    pub address: String,

    /// How often to send metrics to this backend, ms
    pub interval: u64,

    /// How much to sleep when connection to backend fails, ms
    pub connect_delay: u64,

    /// Multiply delay to this value for each consequent connection failure
    pub connect_delay_multiplier: f32,

    /// Maximum retry delay, ms
    pub connect_delay_max: u64,

    /// How much times to retry when sending data to backend before giving up and dropping all metrics
    /// note, that 0 means 1 try
    pub send_retries: usize,
}

impl Default for Carbon {
    fn default() -> Self {
        Self {
            //            enabled: true,
            address: "127.0.0.1:2003".to_string(),
            interval: 30000,
            connect_delay: 250,
            connect_delay_multiplier: 2f32,
            connect_delay_max: 10000,
            send_retries: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub(crate) struct Network {
    /// Address and UDP port to listen for statsd metrics on
    pub listen: SocketAddr,

    /// Address and port for replication server to listen on
    pub peer_listen: SocketAddr,

    /// Address and port for management server to listen on
    pub mgmt_listen: SocketAddr,

    /// UDP buffer size for single packet. Needs to be around MTU. Packet's bytes after that value
    /// may be lost
    pub bufsize: usize,

    /// Enable multimessage(recvmmsg) mode
    pub multimessage: bool,

    /// Number of multimessage packets to receive at once if in multimessage mode
    pub mm_packets: usize,

    /// Nmber of green threads for single-message mode
    pub greens: usize,

    /// Socket pool size for single-message mode
    pub snum: usize,

    /// List of nodes to replicate metrics to
    pub nodes: Vec<String>,

    /// Interval to send snapshots to nodes, ms
    pub snapshot_interval: usize,
}

impl Default for Network {
    fn default() -> Self {
        Self {
            listen: "127.0.0.1:8125".parse().unwrap(),
            peer_listen: "127.0.0.1:8136".parse().unwrap(),
            mgmt_listen: "127.0.0.1:8137".parse().unwrap(),
            bufsize: 1500,
            multimessage: false,
            mm_packets: 100,
            greens: 4,
            snum: 4,
            nodes: Vec::new(),
            snapshot_interval: 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub(crate) struct Consul {
    /// Start in disabled leader finding mode
    pub start_as: ConsensusState,

    /// Consul agent address
    pub agent: SocketAddr,

    /// TTL of consul session, ms (consul cannot set it to less than 10s)
    pub session_ttl: usize,

    /// How often to renew consul session, ms
    pub renew_time: usize,

    /// Name of ke to be locked in consul
    pub key_name: String,
}

impl Default for Consul {
    fn default() -> Self {
        Self {
            start_as: ConsensusState::Disabled,
            agent: "127.0.0.1:8500".parse().unwrap(),
            session_ttl: 11000,
            renew_time: 1000,
            key_name: "service/bioyino/lock".to_string(),
        }
    }
}

#[derive(Debug)]
pub enum Command {
    Daemon,
    Query(MgmtCommand, String),
}

impl System {
    pub fn load() -> (Self, Command) {
        // This is a first copy of args - with the "config" option
        let app = app_from_crate!()
            .arg(
                Arg::with_name("config")
                    .help("configuration file path")
                    .long("config")
                    .short("c")
                    .required(true)
                    .takes_value(true)
                    .default_value("/etc/bioyino/bioyino.toml"),
            )
            .arg(
                Arg::with_name("verbosity")
                    .short("v")
                    .help("logging level")
                    .takes_value(true),
            )
            .subcommand(
                SubCommand::with_name("query")
                    .about("send a management command to running bioyino server")
                    .arg(
                        Arg::with_name("host")
                            .short("h")
                            .default_value("127.0.0.1:8137"),
                    )
                    .subcommand(SubCommand::with_name("status").about("get server state"))
                    .subcommand(
                        SubCommand::with_name("consensus")
                            .arg(Arg::with_name("action").index(1))
                            .arg(
                                Arg::with_name("leader_action")
                                    .index(2)
                                    .default_value("unchanged"),
                            ),
                    ),
            )
            .get_matches();

        let config = value_t!(app.value_of("config"), String).expect("config file must be string");
        let mut file = File::open(&config).expect(&format!("opening config file at {}", &config));
        let mut config_str = String::new();
        file.read_to_string(&mut config_str)
            .expect("reading config file");
        let mut system: System = toml::de::from_str(&config_str).expect("parsing config");

        if let Some(v) = app.value_of("verbosity") {
            system.verbosity = v.into()
        }

        if let Some(query) = app.subcommand_matches("query") {
            let server = value_t!(query.value_of("host"), String).expect("bad server");
            if let Some(_) = query.subcommand_matches("status") {
                (system, Command::Query(MgmtCommand::Status, server))
            } else if let Some(args) = query.subcommand_matches("consensus") {
                let c_action = value_t!(args.value_of("action"), ConsensusAction)
                    .expect("bad consensus action");
                let l_action = value_t!(args.value_of("leader_action"), LeaderAction)
                    .expect("bad leader action");
                (
                    system,
                    Command::Query(MgmtCommand::ConsensusCommand(c_action, l_action), server),
                )
            } else {
                // shold be unreachable
                unreachable!("clap bug?")
            }
        } else {
            (system, Command::Daemon)
        }
    }
}
