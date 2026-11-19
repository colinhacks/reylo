create type mood as enum ('a');
create domain kept_mood as mood;
comment on domain kept_mood is 'a mood kept';
revoke all on domain kept_mood from public;
create domain new_mood as mood;
create table t (id integer primary key, k kept_mood, n new_mood);
