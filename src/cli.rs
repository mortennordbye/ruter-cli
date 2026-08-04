//! Command line surface.

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "ruter",
    version,
    about = "Neste avgang med buss, trikk, T-bane og tog \u{2014} fra der du st\u{e5}r",
    long_about = None,
    after_help = "Eksempler:\n  \
        ruter config add hjem \"Dronningens gate 40, Oslo\"\n  \
        ruter hjem                 reise fra der du er n\u{e5} til \"hjem\"\n  \
        ruter Brekkelia 3D         adresser trenger ikke anf\u{f8}rselstegn\n  \
        ruter hjem --watch         samme, men oppdaterer seg selv\n  \
        ruter near                 avganger fra holdeplasser i n\u{e6}rheten\n  \
        ruter --from jobb hjem     reise mellom to lagrede steder\n  \
        ruter where                sjekk posisjonen og hvor den kommer fra\n\n\
        Faste reiseveier:\n  \
        ruter route add sognsvann --to Sognsvann --via \"Ullev\u{e5}l stadion, Oslo\"\n  \
        ruter sognsvann            kj\u{f8}r den lagrede reiseveien"
)]
pub struct Cli {
    /// Destination: a saved place, "lat,lon", or an address to look up.
    /// Defaults to `default_destination` from the config.
    ///
    /// Collected as words and joined, so an address with spaces needs no quotes:
    /// `ruter Brekkelia 3D` and `ruter "Brekkelia 3D"` are the same thing. A
    /// subcommand still wins the first word, so `ruter near` is unaffected.
    #[arg(value_name = "DESTINASJON")]
    pub destination: Vec<String>,

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

    /// Manage saved routes that go via specific stops.
    Route {
        #[command(subcommand)]
        action: RouteAction,
    },

    /// Show where ruter thinks you are, and why. Useful when GPS misbehaves.
    Where,

    /// Check for a newer version and install it.
    Upgrade {
        /// Only report whether a new version exists.
        #[arg(long)]
        check: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum RouteAction {
    /// Save a route. Run it afterwards with `ruter <navn>`.
    ///
    /// The start comes from the global `--from`; omit it to have the route
    /// begin wherever you happen to be.
    Add {
        /// Short name to refer to it by, e.g. "sognsvann".
        name: String,
        /// Where the route ends.
        #[arg(long, value_name = "STED")]
        to: String,
        /// A stop to travel via. Repeat it, in the order you pass through them.
        #[arg(long, value_name = "HOLDEPLASS")]
        via: Vec<String>,
        /// Take the first match without asking.
        #[arg(short, long)]
        yes: bool,
    },
    /// List saved routes.
    List,
    /// Remove a saved route.
    Remove { name: String },
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// A variadic positional sitting next to subcommands is the kind of thing
    /// clap only complains about at runtime, so assert the definition is sound.
    #[test]
    fn the_definition_is_internally_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn a_destination_may_be_several_words() {
        let cli = Cli::parse_from(["ruter", "Brekkelia", "3D"]);
        assert_eq!(cli.destination, ["Brekkelia", "3D"]);
        assert!(cli.command.is_none());
    }

    #[test]
    fn one_word_and_no_word_destinations_still_parse() {
        assert_eq!(Cli::parse_from(["ruter", "hjem"]).destination, ["hjem"]);
        assert!(Cli::parse_from(["ruter"]).destination.is_empty());
    }

    /// The whole risk of a variadic positional: it must not swallow `near`.
    #[test]
    fn subcommands_still_win_the_first_word() {
        assert!(matches!(Cli::parse_from(["ruter", "where"]).command, Some(Command::Where)));
        assert!(matches!(Cli::parse_from(["ruter", "near"]).command, Some(Command::Near { .. })));
        assert!(Cli::parse_from(["ruter", "where"]).destination.is_empty());
    }

    #[test]
    fn flags_survive_a_multi_word_destination() {
        let cli = Cli::parse_from(["ruter", "--from", "jobb", "Brekkelia", "3D", "--watch"]);
        assert_eq!(cli.common.from.as_deref(), Some("jobb"));
        assert_eq!(cli.destination, ["Brekkelia", "3D"]);
        assert!(cli.common.watch);
    }

    /// `config add` already joined its words; that must keep working.
    #[test]
    fn config_add_still_takes_an_unquoted_address() {
        let cli = Cli::parse_from(["ruter", "config", "add", "hjem", "Brekkelia", "3D"]);
        let Some(Command::Config { action: ConfigAction::Add { name, query, .. } }) = cli.command
        else {
            panic!("expected `config add`");
        };
        assert_eq!(name, "hjem");
        assert_eq!(query.join(" "), "Brekkelia 3D");
    }
}
