-- MySQL aggregates a call in the query block it is written in unless every column its arguments
-- read is bound outside that block; it then belongs to the innermost block that binds one. So a
-- scalar subquery's `count(b.name)` over a column of the query around it is that query's, which is
-- then an aggregate with no GROUP BY: one row, NULL in its bare columns where it reads no row.
-- `nobody` never holds a row. A call over a column of the subquery's own FROM, or over none, is
-- the subquery's. Without ONLY_FULL_GROUP_BY (the `sql_mode` file), a bare column may stand beside.
create table users (id bigint primary key, name varchar(20) not null);
create table posts (id bigint primary key, user_id bigint not null, title varchar(20) not null);
create table comments (id bigint primary key, post_id bigint not null, body varchar(20) not null, created_at datetime not null);
create table nobody (id bigint primary key, name varchar(20) not null, check (false));
