// Statements reylo does not type. `tsc -p lab/pg-forms/untyped --runExternalCode` must fail with
// exactly the errors of expected.txt: one `reylo10` on copy.sql, a COPY through the connection,
// on the statement's own text; one `reylo1` on default_param.sql, at its placeholder, since
// Postgres binds no parameter in an ALTER TABLE; and one `reylo3` on repeated.sql, at the line
// and column of the second :q: each parameter appears once in a statement.
import copy from "./copy.sql";
import defaultParam from "./default_param.sql";
import repeated from "./repeated.sql";
export { copy, defaultParam, repeated };
