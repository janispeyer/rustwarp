use std::borrow::Borrow;

use derive_more::{Display, FromStr};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};

use crate::content::WareID;

// FUTURE: Could be represneted as an enum, discriminating on the first char being '/' or '$'
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, FromStr, Display)]
pub struct SandboxPort(pub String);

impl Borrow<String> for SandboxPort {
	fn borrow(&self) -> &String {
		&self.0
	}
}

#[derive(Clone, Debug, SerializeDisplay, DeserializeFromStr, catverters_derive::Stringoid)]
pub enum Mount {
	#[discriminant = "ro"]
	ReadOnly(String),

	#[discriminant = "rw"]
	ReadWrite(String),

	#[discriminant = "overlay"]
	Overlay(String),
}

#[derive(Clone, Debug, SerializeDisplay, DeserializeFromStr, catverters_derive::Stringoid)]
pub enum FormulaInput {
	#[discriminant = "ware"]
	Ware(WareID),

	#[discriminant = "mount"]
	Mount(Mount),

	#[discriminant = "literal"]
	Literal(String),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GatherDirective {
	pub from: SandboxPort,
	pub packtype: Option<crate::content::Packtype>,
	// TODO:
	// filters: Option<FilterMap>,
}

/// Specifies which image to use to execute the formula.
///
/// Not (yet) part of the official specification!
///
/// Added because pulling images from a registry seems to make more sense
/// than generating rootfs ourselves. An [OCI Registry] provides a hash over
/// the image manifest (which includes hashes to all contents). And we can
/// pull images by their manifest digest from the registry for replays.
///
/// [OCI Registry]: https://github.com/opencontainers/distribution-spec/blob/main/spec.md
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Image {
	/// OCI Reference to an image. This has to include registry and repository and
	/// it may include tag and manifest digest.
	pub reference: String,
	/// Determines if the rootfs will be mounted with readonly or readwrite permissions.
	pub readonly: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Action {
	#[serde(rename = "echo")]
	Echo,
	#[serde(rename = "exec")]
	Execute(ActionExecute),
	#[serde(rename = "script")]
	Script(ActionScript),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActionExecute {
	pub command: Vec<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub network: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActionScript {
	pub interpreter: String,
	pub contents: Vec<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub network: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum FormulaCapsule {
	#[serde(rename = "formula.v1")]
	V1(Formula),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Formula {
	pub image: Image,
	pub inputs: IndexMap<SandboxPort, FormulaInput>,
	pub action: Action,
	pub outputs: IndexMap<crate::plot::LocalLabel, GatherDirective>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum FormulaContextCapsule {
	#[serde(rename = "context.v1")]
	V1(FormulaContext),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, FromStr, Display)]
pub struct WarehouseAddr(pub String);

impl Borrow<String> for WarehouseAddr {
	fn borrow(&self) -> &String {
		&self.0
	}
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FormulaContext {
	pub warehouses: IndexMap<crate::content::WareID, WarehouseAddr>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FormulaAndContext {
	pub formula: FormulaCapsule,
	pub context: FormulaContextCapsule,
}

#[cfg(test)]
mod tests {
	use super::*;

	use crate::test_common::assert_eq_json_roundtrip;
	use expect_test::expect;

	#[test]
	fn test_formulat_roundtrip() {
		let expect = expect![[r#"
{
  "formula": {
    "formula.v1": {
      "image": {
        "reference": "docker.io/busybox:latest",
        "readonly": true
      },
      "inputs": {
        "/": "ware:tar:4z9DCTxoKkStqXQRwtf9nimpfQQ36dbndDsAPCQgECfbXt3edanUrsVKCjE9TkX2v9",
        "/some/ro/path": "mount:ro:/host/readonly/path",
        "/some/rw/path": "mount:rw:/host/readwrite/path",
        "/some/overlay/path": "mount:overlay:/host/overlay/path"
      },
      "action": {
        "exec": {
          "command": [
            "/bin/sh",
            "-c",
            "echo hello from warpforge!"
          ]
        }
      },
      "outputs": {}
    }
  },
  "context": {
    "context.v1": {
      "warehouses": {
        "tar:4z9DCTxoKkStqXQRwtf9nimpfQQ36dbndDsAPCQgECfbXt3edanUrsVKCjE9TkX2v9": "https://warpsys.s3.amazonaws.com/warehouse/4z9/DCT/4z9DCTxoKkStqXQRwtf9nimpfQQ36dbndDsAPCQgECfbXt3edanUrsVKCjE9TkX2v9"
      }
    }
  }
}"#]];
		assert_eq_json_roundtrip::<FormulaAndContext>(&expect);
	}
}
