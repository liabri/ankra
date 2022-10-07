extern crate ankra_wayland;
mod logger;

use std::fs::read_to_string;
use anyhow::Result;

fn main() -> Result<()> {
	logger::init("debug").map_err(|err| eprintln!("logger failed to initialise: {:?}", err)).unwrap();
	let path = xdg::BaseDirectories::with_prefix("ankra")?.get_data_home().join("current_layout");
	let id = read_to_string(&path).map_err(|_| log::error!("No layout set at $XDG_DATA_HOME/ankra/current_layout")).unwrap();

	let mut state = ankra_wayland::State::new(&id);
	state.run();
	Ok(())
}