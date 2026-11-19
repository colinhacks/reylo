drop function "f|g"("mood|kind");
drop view "v|w";
alter type "mood|kind" rename to "mood|kind__old";
create type "mood|kind" as enum ('c', 'a', 'b');
alter domain "dom,x" rename to "dom,x__old";
create domain "dom,x" as "mood|kind";
alter table "u,v" alter column "w,z" type "dom,x" using "w,z"::text::"dom,x";
alter table "t|x" alter column "c|y" drop default;
alter table "t|x" alter column "c|y" type "mood|kind" using "c|y"::text::"mood|kind";
alter table "t|x" alter column "c|y" set default 'a'::"mood|kind";
alter table "t|x" alter column "d.e" type "mood|kind" using "d.e"::text::"mood|kind";
drop domain "dom,x__old";
drop type "mood|kind__old";
create view "v|w" as select "c|y" from "t|x";
create function "f|g"(m "mood|kind") returns int language plpgsql as $$
begin
	return 1;
end $$;
