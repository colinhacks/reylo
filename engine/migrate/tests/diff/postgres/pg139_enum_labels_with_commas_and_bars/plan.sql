alter type tag rename to tag__old;
create type tag as enum ('d|e', 'a', 'b, c', 'f');
alter table t alter column g type tag using g::text::tag;
drop type tag__old;
