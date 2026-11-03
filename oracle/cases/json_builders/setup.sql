-- JSON arrays built of values, the SQL-standard JSON_OBJECT and JSON_ARRAY, and array_to_json
-- of an array a subquery or an aggregate builds: typed by what they are built of.
create table member (id bigint primary key, name text not null, bio text, tags text[] not null, joined timestamptz not null);
create table post (id bigint primary key, member_id bigint not null references member (id), title text not null, body text);
-- Domains over types JSON writes by their own rules: each item is written as its base type.
create domain flags as bit(3);
create domain address as inet;
create domain ref as oid;
create domain spot as point;
create table tagged (id bigint primary key, f flags not null, a address not null, r ref not null, s spot, p point);
