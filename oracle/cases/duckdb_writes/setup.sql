-- What a DuckDB write returns: no row without RETURNING, the row it wrote with one, and as many
-- rows as it wrote; a parameter written bare to a column that may hold NULL takes NULL.
create sequence ids;
create table notes (id integer primary key default nextval('ids'), body varchar not null, tag varchar, created timestamp not null default current_timestamp);
