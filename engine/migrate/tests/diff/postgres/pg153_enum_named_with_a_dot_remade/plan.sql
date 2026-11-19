alter type "mood.x" rename to "mood.x__old";
create type "mood.x" as enum ('a');
alter table t alter column m type "mood.x" using m::text::"mood.x";
drop type "mood.x__old";
