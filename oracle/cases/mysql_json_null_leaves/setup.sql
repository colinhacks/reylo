-- A JSON value may be JSON's null, which a NOT NULL JSON column holds as the document `null`. A
-- document built around it holds that as a null, in an object, an array or an aggregate, through a
-- view, a derived table and a nested object alike, so its place there is typed nullable; at the
-- top of a column it is a value. A JSON Schema a CHECK holds the column to may refuse null, and
-- then the place is not nullable.
create table events (id int primary key, kind varchar(10) not null, payload json not null);
insert into events values (1, 'ping', 'null'), (2, 'pong', '{"a": 1}');
create view event_payloads as select id, payload from events;
create table shaped (id int primary key, doc json not null, check (json_schema_valid('{"type": "object"}', doc)));
insert into shaped values (1, '{}');
