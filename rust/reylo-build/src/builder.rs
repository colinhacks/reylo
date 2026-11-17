//! The modules `reylo generate rust-sqlx` writes, written by a build script instead. Each input is
//! read from the crate's directory, `src/` where none is given, and every module goes into the one
//! file in Cargo's `OUT_DIR`, a module per directory under its input. Cargo runs the script again
//! when a `.sql` file, the project file, a `.env` file or the project's migrations change.
//!
//! A statement that cannot be typed fails the build with one of Cargo's own errors, at its file,
//! line and column, and no file is written:
//!
//! ```text
//! error: app@0.1.0: src/queries/get_user.sql:1:12: column "nme" does not exist (42703)
//! error: build script logged errors
//! ```

use crate::generate::Generator;
use std::path::PathBuf;

/// The modules of sqlx queries, as `#[reylo::query]` writes them, for a build script:
///
/// ```no_run
/// // build.rs
/// fn main() {
///     reylo_build::rust_sqlx().input("src/queries").out("queries.rs").run();
/// }
/// ```
pub fn rust_sqlx() -> RustSqlx {
    RustSqlx(crate::generate::rust_sqlx().in_build_script())
}

/// The `.sql` files a build script reads and the file it writes. Made by [`rust_sqlx`].
#[derive(Clone, Debug)]
pub struct RustSqlx(Generator);

impl RustSqlx {
    /// A `.sql` file, a directory searched for them, or a glob (`src/**/*.sql`), from the crate's
    /// directory. With none, the crate's `src/`.
    pub fn input(self, path: impl Into<String>) -> Self {
        Self(self.0.input(path))
    }

    /// The file in `OUT_DIR` every module is written to, `queries.rs` where none is given.
    pub fn out(self, file: impl Into<PathBuf>) -> Self {
        Self(self.0.out(file))
    }

    /// Writes the modules, or, where any statement fails, writes none and fails the build with each
    /// failure as a Cargo error (`cargo::error`), which Cargo shows as it shows its own.
    pub fn run(self) {
        if let Err(error) = self.0.run() {
            for line in error.to_string().lines() {
                println!("cargo::error={line}");
            }
        }
    }
}
