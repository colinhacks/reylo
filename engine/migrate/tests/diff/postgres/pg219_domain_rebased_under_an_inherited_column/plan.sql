alter domain d rename to d__old;
create domain d as bigint;
alter table p alter column n type d using n::text::d;
drop domain d__old;
