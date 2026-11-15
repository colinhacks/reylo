create type mood as enum ('a', 'b', 'c'); create table t (a varchar default ('a'::mood)::varchar, c mood);
