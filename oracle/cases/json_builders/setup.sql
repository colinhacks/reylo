-- JSON arrays built of values, the SQL-standard JSON_OBJECT and JSON_ARRAY, and array_to_json
-- of an array a subquery or an aggregate builds: typed by what they are built of.
create table member (id bigint primary key, name text not null, bio text, tags text[] not null, joined timestamptz not null);
create table post (id bigint primary key, member_id bigint not null references member (id), title text not null, body text);
