//! Command line surface.

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "ruter",
    version,
    about = "Neste avgang med buss, trikk, T-bane og tog \u{2014} fra der du st\u{e5}r",
    long_about = None,
    after_help = "Eksempler:\n  \
        ruter config add hjem \"Ullev\u{e5}lsveien 15, Oslo\"\n  \
        ruter hjem                 reise fra der du er n\u{e5} til \"hjem\"\n  \
        ruter hjem --watch         samme, men oppdaterer seg selv\n  \
        ruter near                 avganger fra holdeplasser i n\u{e6}rheten\n  \
        ruter --from jobb hjem     reise mellom to lagrede steder"
)]
pub struct Cli {
    /// Destination: a saved place, "lat,lon", or an address to look up.
    /// Defaults to `default_destination` from the config.
    pub destination: Option<String>,

    #[command(flatten)]
    pub common: Common,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Args, Clone)]
pub struct Common {
    /// Where to start from. Defaults to your current position.
    #[arg(short, long, global = true, value_name = "STED")]
    pub from: Option<String>,

    /// Refresh continuously in a full-screen view.
    #[arg(short, long, global = true)]
    pub watch: bool,

    /// Print raw JSON instead of a rendered board.
    #[arg(long, global = true)]
    pub json: bool,

    /// How many results to show.
    #[arg(short = 'n', long, global = true, value_name = "ANTALL")]
    pub count: Option<usize>,

    /// Limit to these transport modes, e.g. --modes bus,tram
    #[arg(long, global = true, value_delimiter = ',', value_name = "MODUS")]
    pub modes: Option<Vec<String>>,

    /// Do not try Core Location.
    #[arg(long, global = true)]
    pub no_gps: bool,

    /// Do not fall back to IP geolocation.
    #[arg(long, global = true)]
    pub no_ip: bool,

    /// Force colour on or off. Defaults to auto-detection.
    #[arg(long, global = true, value_name = "NÅR", value_parser = ["auto", "always", "never"])]
    pub color: Option<String>,
}

impl Common {
    /// `None` means "decide from the environment".
    pub fn colour_override(&self) -> Option<bool> {
        match self.color.as_deref() {
            Some("always") => Some(true),
            Some("never") => Some(false),
            _ => None,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Departures from stops near you.
    Near {
        /// Search radius in metres.
        #[arg(long, default_value_t = 600)]
        radius: u32,
        /// How many stops to show.
        #[arg(long, default_value_t = 3)]
        stops: usize,
    },

    /// Manage saved places and settings.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Look up an address and save it under a name.
    Add {
        /// Short name to refer to it by, e.g. "hjem".
        name: String,
        /// Address or place to look up.
        query: Vec<String>,
        /// Take the first match without asking.
        #[arg(short, long)]
        yes: bool,
    },
    /// List saved places.
    List,
    /// Remove a saved place.
    Remove { name: String },
    /// Print the path to the config file.
    Path,
}
