create type "mood|kind" as enum ('c', 'a', 'b');
create table "t|x" (id int primary key, "c|y" "mood|kind" default 'a', "d.e" "mood|kind");
create view "v|w" as select "c|y" from "t|x";
create function "f|g"(m "mood|kind") returns int language plpgsql as $$
begin
	return 1;
end $$;
create domain "dom,x" as "mood|kind";
create table "u,v" (id int primary key, "w,z" "dom,x");
