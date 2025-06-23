# mountain-mqtt

This is a monorepo of crates for a `no_std` compatible [MQTT v5](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html) client.

Each folder is a separate rust crate (e.g. `mountain-mqtt` contains the main library crate). This is NOT a cargo workspace, it's just a set of crates that live alongside each other in the repo. This has been done for flexibility, in particular to allow for different targets to be used for each crate if needed - this in turn allows for example projects to build for embedded targets.

Note there are also `README.md` files in each crate folder, covering the respective crate.

For development, crates reference each other via relative paths, each crate is also published to `crates.io`.

## Editing with VS Code

There are two approaches:

1. Open an individual crate as a project, and see just that crate in the VS Code window, or
2. Open the provided `mountain-mqtt.code-workspace` file as a VS Code workspace. This will open each crate as a separate root in the workspace, so they can all be edited together. Rust Analyzer will run on each crate. The contents of the root of the workspace (including this file) are displayed under the `/` entry in the explorer.

## How is the VS Code workspace configured?

This is an interesting one, since we want to be able to see each of the crates as a separate project, and at the same time show the root directory itself.

This is done in the `.code-workspace` file by adding a folder for each crate, then a folder for the root (we can give this a name, we've used `/` for simplicity). This would by default also show the crate folders within the root folder - we can prevent this by specifying file exclusions for each crate:

```json
{
  "folders": [
    {
      "path": "crate-a"
    },
    {
      "path": "crate-b"
    },
    {
      "path": ".",
      "name": "/"
    }
  ],
  "settings": {
    "files.exclude": {
      "crate-a": true,
      "crate-b": true
    }
  }
}
```

You will need to add any new crates to both the `folders` array, and the `files.exclude` setting.

This approach of keeping crates as separate projects without a cargo workspace, and opening them as multiple roots in a VS Code workspace, is [suggested here](https://www.reddit.com/r/rust/comments/14x9q0p/comment/jrm96d4/).The general approach to excluding the crate folders is covered in [this comment on a VS Code issue](https://github.com/microsoft/vscode/issues/82145#issuecomment-859550844).

## Why not a Cargo workspace?

A Cargo workspace currently applies the same target to all projects/crates, and we require general-purpose crates like `mountain-mqtt` to run on either the host PC or embedded.

It sounds like there may be future additions to Cargo and/or [Rust Analyzer](https://github.com/rust-lang/rust-analyzer/issues/13529) to allow for different settings in different projects, but this didn't seem to cover our use case when the project was set up.
