-- An ON CONFLICT (…) whose columns are the key of no unique index: the server prepares the
-- statement and refuses every execution. A partial index needs its predicate in the target, an
-- expression index its expression; a named constraint, a deferrable one, an inheritance parent's
-- index, an index under another collation and a view are no false alarms.
create collation ci (provider = icu, locale = 'und-u-ks-level2', deterministic = false);
create table bar (id bigint primary key, name text, email text, code text, a int, b int, tag text collate ci, unique (a, b));
create unique index bar_email_active on bar (email) where email <> '';
create unique index bar_lower_name on bar (lower(name));
create unique index bar_code_included on bar (code) include (name);
create unique index bar_tag on bar (tag collate "C");
create table parent (id bigint primary key, name text);
create table child () inherits (parent);
create table deferred (id bigint primary key, name text, constraint deferred_name unique (name) deferrable initially immediate);
create table parted (id bigint, region text, primary key (id, region)) partition by list (region);
create table parted_eu partition of parted for values in ('eu', 'a', 'A', 'b', '', 'a ', 'k900000');
create table plain (id bigint, name text);
create view bar_view as select id, name from bar;
create table upserted (k int primary key, v text);
insert into upserted values (2, 'Blue');
