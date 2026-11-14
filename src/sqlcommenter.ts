// The sqlcommenter comment (https://google.github.io/sqlcommenter/spec/) on every statement Reylo sends (src/execute.ts), unless the project file says `sqlcommenter: false`; the query's `sql` leaves it out: `/*file='queries%2Fget_user.sql'*/`, the `.sql` file the statement came from. Datadog's Database Monitoring and PlanetScale's query insights read it as the statement's tags, and `pg_stat_activity` and a slow-query log show it as written. The file is its one tag, so the text is the same on every call, and a driver that keeps its statements prepared by their text keeps this one.
//
// It ends the statement, as the specification has it, except on SQLite, where it opens it. SQLite names a column with no alias after the text of its expression, which runs up to the next token and so takes in a comment before it: `select 1 + 1 /*c*/` returns a column named `1 + 1 /*c*/`, which the row's type does not declare. A comment before the statement's first token is in no column's text. After a closing semicolon it would be in none either, but D1 and a Durable Object refuse that as a second statement that is empty ("SQL code did not contain a statement"), and SQLite's own record of the statement stops at the semicolon.

/** The statement with a comment naming `source`: at its end, before a closing semicolon, and on SQLite at its start. */
export function withSqlcommenter(text: string, source: string, dialect?: string): string {
  // The value is URL-encoded, and a quote, which encodeURIComponent leaves as it is, escaped.
  const tag = `/*file='${encodeURIComponent(source).replace(/'/g, "\\'")}'*/`;
  if (dialect === "sqlite") return `${tag} ${text}`;
  const body = text.replace(/[\s;]+$/, "");
  const semicolon = text.slice(body.length).includes(";") ? ";" : "";
  // A line comment on the statement's last line (`--`, and MySQL's `#`) would take the tag in, so
  // after one the tag goes on a line of its own.
  const last = body.slice(body.lastIndexOf("\n") + 1);
  return `${body}${/--|#/.test(last) ? "\n" : " "}${tag}${semicolon}`;
}
