# The pg_query crate (pganalyze/pg_query.rs, branch pg-18) and the libpg_query commit its submodule
# names, which `engine/build.sh --prepare` fetches from GitHub by commit and checks against these
# SHA-256s (README.md). They are here, among the Postgres crate's hashed sources, so that another
# commit changes the engine's fingerprint as a change to the patch does.
PG_QUERY_COMMIT="ad0c87ed59c84ec6b42c970bb42d7985b7b5878b"
PG_QUERY_SHA256="de2307fc9beb0c37e8da569b48d69778d5c847e8b80be2620655c032eb800043"
LIBPG_QUERY_COMMIT="c1546e7e97edc93fe8474d7a59881e91fbf48cfc"
LIBPG_QUERY_SHA256="7df58cb1d3a65fdb9fe87f64961c3b3664008fb3661d311827da47cb1d313a3e"
