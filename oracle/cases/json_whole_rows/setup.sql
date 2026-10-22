-- A whole row written `t.*` inside JSON the statement builds, an array with a NULL element inside
-- it, the floats and decimals JSON has no number for, and two result columns of one name.
create table teams (id bigint primary key, name text not null, score float8);
create table members (id bigint primary key, team_id bigint references teams (id), name text not null, rating numeric);
