-- An hstore inside a JSON document is an object of its keys, each value text or null, since the
-- extension's cast to JSON is what json_build_object and json_agg apply; outside one it is text.
create extension hstore;
create table settings (id bigint primary key, prefs hstore not null, extra hstore);
