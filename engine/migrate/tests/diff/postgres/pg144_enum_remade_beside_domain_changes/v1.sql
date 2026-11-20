create type mood as enum ('a', 'b');
create domain kept_mood as mood constraint valid check (value is not null);
comment on domain kept_mood is 'a mood kept';
revoke all on domain kept_mood from public;
create table t (id integer primary key, k kept_mood);
