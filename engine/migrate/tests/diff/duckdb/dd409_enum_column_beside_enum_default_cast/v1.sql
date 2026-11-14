create type mood as enum ('a', 'b'); create table t (a varchar default ('a'::mood)::varchar, c mood);
