// The sqlcommenter comment (https://google.github.io/sqlcommenter/spec/) that ends every statement
// a build step writes, unless the project file says `sqlcommenter: false`:
// `/*file='queries%2Fget_user.sql'*/`, the `.sql` file the statement came from. Datadog's Database Monitoring and PlanetScale's query insights read
// it as the statement's tags, and `pg_stat_activity` and a slow-query log show it as written. The
// file is its one tag, so the text is the same on every call, and a driver that keeps its statements
// prepared by their text keeps this one.

/** The statement with a comment naming `source` at its end, before a closing semicolon. */
export function withSqlcommenter(text: string, source: string): string {
  // The value is URL-encoded, and a quote, which encodeURIComponent leaves as it is, escaped.
  const tag = `/*file='${encodeURIComponent(source).replace(/'/g, "\\'")}'*/`;
  const body = text.replace(/[\s;]+$/, "");
  const semicolon = text.slice(body.length).includes(";") ? ";" : "";
  // A line comment on the statement's last line (`--`, and MySQL's `#`) would take the tag in, so
  // after one the tag goes on a line of its own.
  const last = body.slice(body.lastIndexOf("\n") + 1);
  return `${body}${/--|#/.test(last) ? "\n" : " "}${tag}${semicolon}`;
}
