-- DuckDB's NULL: what a table's NOT NULL columns, an outer join, a filter and each kind of
-- expression make of it. DuckDB's own describe marks none of it right, so every column here is
-- the engine's proof.
create table teams (id integer primary key, title varchar not null, budget decimal(12, 2));
create table users (id integer primary key, name varchar not null, email varchar, team_id integer references teams (id), born date, score double);
create table profiles (user_id integer primary key references users (id), profiles struct(handle varchar, bio varchar));
