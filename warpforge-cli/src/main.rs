use clap::Parser;
use std::env;
use std::path;

mod cmds;
mod dab;
mod errors;

use errors::*;

fn main() {
	main2().expect("wow")
	// TODO: destructure the enum members into distinct exit codes.
}

fn main2() -> Result<(), Error> {
	let cli =
		cmds::Root::try_parse().map_err(|e| Error::InvalidArguments { cause: Box::new(e) })?;

	if cli.verbosity >= 2 {
		println!("args: {:?}", cli);
	}

	// Dispatch.
	//
	// Okay, I have some nonjoy at this.  I want:
	//   - 1: to receive the command object with all parents.
	//   - 2: to have a func on my command strugs that receives a call, rather than have to make this dispatch table.
	match &cli.subcommand {
		Some(cmds::Subcommands::Catalog(cmd)) => {
			match &cmd.subcommand {
				cmds::catalog::Subcommands::ReadItem(cmd) => {
					let user_home = env::var("HOME")
						.map_err(|e| Error::BizarreEnvironment { cause: Box::new(e) })?;

					// Create the catalog data access broker.  Store in a box just so we can have dynamic dispatch.  (This is architecture astronauting, but I wanna know that I know how to do this.)
					//TODO: check for a root workspace above $CWD before $HOME/.warphome
					let catalog_handle: Box<dyn dab::catalog::Handle> =
						Box::new(dab::catalog::FsHandle::new(
							path::Path::new(&user_home).join(".warphome/catalogs/warpsys"),
						));

					let catalog_release = catalog_handle
						.load_release(&cmd.catalog_ref.module_name, &cmd.catalog_ref.release_name)
						.map_err(|e| Error::CatalogAccessError { cause: e })?;

					match catalog_release.items.get(&cmd.catalog_ref.item_name) {
						Some(wareid) => {
							println!("{wareid}");
							Ok(())
						}
						None => {
							println!("catalog item not found.");
							Err(Error::CatalogEntryNotExists {
								reference: cmd.catalog_ref.clone(),
							})
						}
					}
				}
			}
		}
		Some(cmds::Subcommands::Ware(cmd)) => match &cmd.subcommand {
			cmds::ware::Subcommands::Unpack(cmd) => {
				panic!("unpack unimplemented...")
			}
		},
		None => {
			println!("command used with no args.  some explanation text should go here :)");
			Ok(())
		}
	}
}

#[test]
fn verify_cli() {
	use clap::CommandFactory;
	cmds::Root::command().debug_assert()
}
